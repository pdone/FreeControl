//! 日志持久化与关键操作记录（design D1–D11）。
//!
//! 目标：把 scrcpy 的输出与本程序自身的操作落到 `%APPDATA%/FreeControl2/logs/<本地日期>.log`，
//! 供事后排查。三条约束决定了本模块的形状：
//! - **唯一的写入者**：所有日志都经 [`LogSink`] 写同一个文件句柄，不给第二处「顺带写日志」的机会；
//! - **数据根沿既有唯一来源**：日志目录取自 [`config::app_data_root`]，本模块不自己算 `%APPDATA%`；
//! - **写盘不参与主流程**：写入路径上没有任何 `unwrap` 与错误上抛 —— 磁盘问题不得改变任何操作的结果，
//!   会话输出即便写盘失败也照常留在内存里供界面查看。
//!
//! 时间取**本地时间**（Windows 用 `GetLocalTime`）：文件名与行时间戳要与用户看到的「今天」一致，
//! 否则 21:00 复现的问题会落到第二天那个文件里。
//!
//! ## 为什么不用 `tauri-plugin-log`
//! 它的 `Target::LogDir` 以 bundle identifier 为目录名（本工程即 `%APPDATA%/com.freecontrol2`），
//! 与 spec 约定的数据根不是一个目录；它的轮转按文件**大小**，而本 spec 的文件名语义是「一天一个文件」。
//! 本模块零新依赖，且命名 / 超期 / 清理都是可单测的纯函数。

use crate::config;
use crate::shell;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::AppHandle;

/// 日志目录名（挂在应用数据根下，与 `config.json`、`scrcpy-*` 同级）。
pub const LOG_DIR_NAME: &str = "logs";

/// 日志级别（写入行内用大写，便于按 `[ERROR]` 直接过滤）。
///
/// 三档是 spec 要求的下限：正常信息 / 可恢复的异常 / 错误。级别**不提供界面配置** ——
/// scrcpy 自身的 `--verbosity`（来源 doc/scrcpy）是运行参数，与此无关。
pub const LEVEL_INFO: &str = "INFO";
pub const LEVEL_WARN: &str = "WARN";
pub const LEVEL_ERROR: &str = "ERROR";

/// `retention_days` 的「永久保留」在原子量里的表示。
const RETENTION_FOREVER: u64 = u64::MAX;

// ── 日期与时刻（纯值类型，便于单测）──────────────────────────

/// 一个本地日期。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

/// 一个本地时刻（精确到毫秒）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalTime {
    pub date: Date,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub millis: u32,
}

impl Date {
    /// 文件名里用的日期段：`yyyy-MM-dd`。
    pub fn name(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// 严格解析 `yyyy-MM-dd`（月日必须真实存在）。解析不出来即「不是本程序写出的文件」。
    pub fn parse(text: &str) -> Option<Date> {
        let bytes = text.as_bytes();
        if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return None;
        }
        let num = |s: &str| -> Option<u32> {
            if !s.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            s.parse::<u32>().ok()
        };
        let year = num(&text[0..4])? as i32;
        let month = num(&text[5..7])?;
        let day = num(&text[8..10])?;
        if month < 1 || month > 12 || day < 1 || day > days_in_month(year, month) {
            return None;
        }
        Some(Date { year, month, day })
    }

    /// 自 1970-01-01 起的天数（只用于比较，不涉及时区）。
    fn days(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// 民用历 → 自 1970-01-01 的天数（Howard Hinnant 的算法，纯整数）。
fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year } as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let m = month as i64;
    let d = day as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146097 + doe - 719468
}

/// 自 1970-01-01 的天数 → 民用历。
///
/// 只在单测里当「反函数」用（验证 `days_from_civil` 的往返一致）；生产代码取本地时刻走
/// Windows 的 `GetLocalTime` / 非 Windows 的 `chrono::Local`，不再需要从时间戳反推民用历，
/// 故只在测试构建下参与编译。
#[cfg(test)]
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y } as i32, m, d)
}

/// 当前本地时刻。Windows 走 `GetLocalTime`（拆好的字段，无需引时间库）。
#[cfg(windows)]
pub fn local_now() -> LocalTime {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;

    // SAFETY: `GetLocalTime` 只写它接收的那一个 `SYSTEMTIME`，指针指向本函数栈上的变量。
    let mut st: SYSTEMTIME = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut st) };
    LocalTime {
        date: Date {
            year: st.wYear as i32,
            month: st.wMonth as u32,
            day: st.wDay as u32,
        },
        hour: st.wHour as u32,
        minute: st.wMinute as u32,
        second: st.wSecond as u32,
        millis: st.wMilliseconds as u32,
    }
}

/// 非 Windows 平台：`chrono::Local` 读**系统时区**给出真正的本地时刻。
///
/// 不再退化为 UTC —— 日志按「本地零点」切天、行时间戳要与用户看到的「今天 / 现在」一致，
/// UTC 会让这两件事在非 UTC 时区全错（tasks 4.4 / spec `logging`「本地时间」）。
/// `Local` 经 `iana-time-zone` 读系统时区（`/etc/localtime` 或 `TZ`），纯 Rust 无 C 构建依赖。
#[cfg(not(windows))]
pub fn local_now() -> LocalTime {
    use chrono::{Datelike, Timelike};

    let now = chrono::Local::now();
    let date = now.date_naive();
    let time = now.time();
    LocalTime {
        date: Date {
            year: date.year(),
            month: date.month(),
            day: date.day(),
        },
        hour: time.hour(),
        minute: time.minute(),
        second: time.second(),
        millis: time.nanosecond() / 1_000_000,
    }
}

/// 当前本地日期。
pub fn today() -> Date {
    local_now().date
}

// ── 目录、命名与行格式（纯函数）──────────────────────────────

/// 日志目录：应用数据根下的 `logs`。
pub fn log_dir(root: &Path) -> PathBuf {
    root.join(LOG_DIR_NAME)
}

/// 某一天的日志文件名：`yyyy-MM-dd.log`。
pub fn log_file_name(date: &Date) -> String {
    format!("{}.log", date.name())
}

/// 日志目录下某一天的文件路径。路径拼接只有这一处，写入与断言不会各拼一遍。
pub fn log_path_in(dir: &Path, date: Date) -> PathBuf {
    dir.join(log_file_name(&date))
}

/// 文件名 → 日期；不是本程序写出的日志文件名则为 `None`。
fn parse_log_file_name(name: &str) -> Option<Date> {
    Date::parse(name.strip_suffix(".log")?)
}

/// 一行日志：`yyyy-MM-dd HH:mm:ss.SSS [级别] [来源] 消息`。
pub fn format_line(at: &LocalTime, level: &str, source: &str, msg: &str) -> String {
    format!(
        "{} {:02}:{:02}:{:02}.{:03} [{}] [{}] {}",
        at.date.name(),
        at.hour,
        at.minute,
        at.second,
        at.millis,
        level,
        source,
        msg
    )
}

// ── 保留策略与清理（纯函数）──────────────────────────────────

/// 保留策略的展示文案（写进启动行，便于从日志本身看出当时的策略）。
pub fn retention_label(days: Option<u32>) -> String {
    match days {
        Some(d) => format!("保留 {d} 天"),
        None => "永久保留".to_string(),
    }
}

/// 文件名是否已超出保留期。
///
/// 判定只用**文件名里的日期**，不看文件系统的修改时间 —— 后者会被复制 / 解压 / 同步改写，
/// 会让「刚被同步过的一份旧日志」逃过清理或反过来误删；文件名里的日期是写入时就定下的事实。
/// 不匹配 `yyyy-MM-dd.log` 的文件一律不算超期（用户放进目录里的东西不碰）。
pub fn is_expired(name: &str, today: Date, retention_days: Option<u32>) -> bool {
    let days = match retention_days {
        Some(d) => d,
        None => return false,
    };
    match parse_log_file_name(name) {
        Some(date) => today.days() - date.days() >= days as i64,
        None => false,
    }
}

/// 清理超期日志，返回删除的文件数；目录不存在时为 0（且不报错）。
pub fn cleanup(dir: &Path, today: Date, retention_days: Option<u32>) -> usize {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };
    let mut removed = 0usize;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !is_expired(&name, today, retention_days) {
            continue;
        }
        if fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

// ── 写入者 ──────────────────────────────────────────────────

/// 日志写入者：持有目录、当前保留策略与**当前打开的那一个文件**。
///
/// 结构体可独立构造（[`LogSink::open_in`]），单测直接用它；进程内单例只是它的薄封装。
pub struct LogSink {
    dir: PathBuf,
    /// 保留天数；`RETENTION_FOREVER` 表示永久保留。
    retention_days: AtomicU64,
    /// 当前打开的文件（日期 + 句柄）；日期变了就换文件。
    cur: Mutex<Option<(Date, BufWriter<File>)>>,
}

fn encode_retention(days: Option<u32>) -> u64 {
    match days {
        Some(d) => d as u64,
        None => RETENTION_FOREVER,
    }
}

fn decode_retention(v: u64) -> Option<u32> {
    if v == RETENTION_FOREVER {
        None
    } else {
        Some(v as u32)
    }
}

impl LogSink {
    pub fn open_in(dir: PathBuf, retention_days: Option<u32>) -> Self {
        LogSink {
            dir,
            retention_days: AtomicU64::new(encode_retention(retention_days)),
            cur: Mutex::new(None),
        }
    }

    /// 当前保留天数；`None` = 永久保留。
    pub fn retention(&self) -> Option<u32> {
        decode_retention(self.retention_days.load(Ordering::Relaxed))
    }

    /// 换保留策略并立即按新策略清理一次（用户改完策略就生效，不必等下次启动）。
    pub fn set_retention(&self, days: Option<u32>) {
        self.retention_days.store(encode_retention(days), Ordering::Relaxed);
        self.cleanup_now();
    }

    /// 立即按当前策略清理一次。
    pub fn cleanup_now(&self) {
        let days = decode_retention(self.retention_days.load(Ordering::Relaxed));
        let _ = cleanup(&self.dir, today(), days);
    }

    /// 写入一行（用当前本地时间）。
    pub fn write(&self, level: &str, source: &str, msg: &str) {
        self.write_at(local_now(), level, source, msg);
    }

    /// 按给定时刻写入一行。
    ///
    /// 写入路径上**不返回错误**：拿不到锁就丢弃这一行（锁中毒时取回内部值，绝不让记日志这件事
    /// panic 任何线程）、建目录 / 开文件失败就这一行没写出去、`flush` 失败就把句柄置空以便下次重开。
    fn write_at(&self, at: LocalTime, level: &str, source: &str, msg: &str) {
        let line = format_line(&at, level, source, msg);
        let mut cur = match self.cur.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        let need_open = match cur.as_ref() {
            Some((date, _)) => *date != at.date,
            None => true,
        };
        if need_open {
            // 换文件：先把旧句柄放回（drop 时收尾），再按新日期开。
            *cur = None;
            if let Ok(file) = open_day_file(&self.dir, at.date) {
                *cur = Some((at.date, file));
            }
            // 进入新的一天时顺带清理一次（启动后的第一次写入同样走这里）。
            let days = decode_retention(self.retention_days.load(Ordering::Relaxed));
            let _ = cleanup(&self.dir, at.date, days);
        }
        if let Some((_, w)) = cur.as_mut() {
            let _ = writeln!(w, "{line}");
            if w.flush().is_err() {
                // 磁盘写不动：置空句柄，下一次写入重开重试。
                *cur = None;
            }
        }
    }
}

fn open_day_file(dir: &Path, date: Date) -> std::io::Result<BufWriter<File>> {
    fs::create_dir_all(dir)?;
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path_in(dir, date))?;
    Ok(BufWriter::new(file))
}

// ── 进程内单例 ──────────────────────────────────────────────

/// 进程内唯一写入者。埋点大多发生在**拿不到 `AppHandle`** 的地方（会话的输出读取线程、
/// 设备状态订阅线程、下载任务），单例让它们退化成一行函数调用，而不必把 sink 穿进每个签名。
static SINK: OnceLock<LogSink> = OnceLock::new();

/// 初始化日志（幂等）：解析目录、按保留策略清理一次、写启动行。
///
/// 保留策略来自配置（`logging.retention`），故必须在 `load_config` 之后调用。
pub fn init(root: &Path, retention_days: Option<u32>) {
    let dir = log_dir(root);
    let _ = SINK.set(LogSink::open_in(dir.clone(), retention_days));
    if let Some(sink) = SINK.get() {
        sink.cleanup_now();
        sink.write(
            LEVEL_INFO,
            "app",
            // &format!(
            //     "应用启动 v{} · 数据目录 {} · 日志目录 {} · {}",
            //     env!("CARGO_PKG_VERSION"),
            //     root.display(),
            //     dir.display(),
            //     retention_label(retention_days)
            &format!(
                "应用启动 v{}",
                env!("CARGO_PKG_VERSION")
            ),
        );
    }
}

/// 保留策略变更：记一行并立即按新策略清理一次。
///
/// **值没变就什么都不做** —— `save_config` 每次保存（含任何一处参数改动）都会调到这里，
/// 不做这个判定的话，日志会被「保留策略改为…」刷满，且每次保存都白扫一遍目录。
pub fn set_retention(days: Option<u32>) {
    if let Some(sink) = SINK.get() {
        if sink.retention() == days {
            return;
        }
        sink.set_retention(days);
        sink.write(
            LEVEL_INFO,
            "app",
            &format!("日志保留策略改为「{}」", retention_label(days)),
        );
    }
}

pub fn info(source: &str, msg: impl AsRef<str>) {
    log(LEVEL_INFO, source, msg.as_ref());
}

pub fn warn(source: &str, msg: impl AsRef<str>) {
    log(LEVEL_WARN, source, msg.as_ref());
}

pub fn error(source: &str, msg: impl AsRef<str>) {
    log(LEVEL_ERROR, source, msg.as_ref());
}

/// 按给定级别写一行（级别是运行时决定时用它，如会话结束行按退出码分档）。
///
/// 未初始化时静默丢弃 —— 日志不可用不得影响任何调用方。
pub fn log(level: &str, source: &str, msg: &str) {
    if let Some(sink) = SINK.get() {
        sink.write(level, source, msg);
    }
}

/// 在系统文件管理器中定位日志目录（不存在则先创建）。
///
/// 与 `assets::open_scrcpy_dir` 同款行为：界面只给「打开日志目录」这一个入口，
/// 故不提供「把日志目录路径交给前端」的命令 —— 没有第二处用途。
/// 打开动作本身见 `shell`（Windows 上是一次 `ShellExecuteW`，`explore` 动词）。
#[tauri::command]
pub fn open_log_dir(app: AppHandle) -> Result<(), String> {
    let root = config::app_data_root(&app)?;
    let dir = log_dir(&root);
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    shell::open_dir(&dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "fcn-log-{}-{}-{}",
            tag,
            std::process::id(),
            nanos
        ))
    }

    fn d(year: i32, month: u32, day: u32) -> Date {
        Date { year, month, day }
    }

    fn read(path: &Path) -> String {
        let mut s = String::new();
        File::open(path).unwrap().read_to_string(&mut s).unwrap();
        s
    }

    #[test]
    fn file_naming_uses_local_date() {
        assert_eq!(log_file_name(&d(2026, 9, 28)), "2026-09-28.log");
        // 月日补零：文件名恒为 10 位日期段 + `.log`
        assert_eq!(log_file_name(&d(2026, 1, 5)), "2026-01-05.log");
        assert_eq!(
            log_path_in(&log_dir(Path::new("C:/data")), d(2026, 9, 28)),
            PathBuf::from("C:/data").join(LOG_DIR_NAME).join("2026-09-28.log")
        );
    }

    #[test]
    fn line_format_has_time_level_source_message() {
        let at = LocalTime {
            date: d(2026, 9, 28),
            hour: 21,
            minute: 3,
            second: 7,
            millis: 42,
        };
        assert_eq!(
            format_line(&at, LEVEL_INFO, "session", "启动会话 pid=1"),
            "2026-09-28 21:03:07.042 [INFO] [session] 启动会话 pid=1"
        );
    }

    #[test]
    fn only_own_file_names_are_recognized() {
        assert_eq!(Date::parse("2026-09-28"), Some(d(2026, 9, 28)));
        // 非本程序命名的文件一律不认（清理时也就不可能删到它们）
        for name in [
            "2026-9-28",       // 月日未补零
            "2026-13-01",      // 月份不存在
            "2026-02-30",      // 该月没有这一天
            "2023-02-29",      // 平年没有 2 月 29 日
            "notes.txt",
            "backup.log",
            "2026-09-28.log.bak",
            "2026-09-28.log",
            "xxxx-09-28",
        ] {
            assert_eq!(Date::parse(name), None, "{name} 不应被解析为日期");
        }
        assert_eq!(Date::parse("2024-02-29"), Some(d(2024, 2, 29)));
    }

    #[test]
    fn expiry_boundary_is_the_retention_window() {
        let today = d(2026, 9, 28);
        // 保留 7 天 = 09-22..09-28 这 7 天；09-21 正好落在窗口外
        assert!(!is_expired("2026-09-22.log", today, Some(7)));
        assert!(!is_expired("2026-09-28.log", today, Some(7)));
        assert!(is_expired("2026-09-21.log", today, Some(7)));
        assert!(!is_expired("2026-09-21.log", today, Some(30)));
        // 永久保留：什么都不删
        assert!(!is_expired("2000-01-01.log", today, None));
        // 不是本程序的文件：永远不删
        assert!(!is_expired("backup.log", today, Some(7)));
    }

    #[test]
    fn cleanup_touches_only_own_expired_files() {
        let root = temp_dir("cleanup");
        let dir = log_dir(&root);
        fs::create_dir_all(&dir).unwrap();
        for name in [
            "2020-01-01.log",
            "2026-09-21.log",
            "2026-09-22.log",
            "2026-09-28.log",
            "backup.log",
            "notes.txt",
            "2026-09-28.log.bak",
        ] {
            fs::write(dir.join(name), b"x").unwrap();
        }

        let removed = cleanup(&dir, d(2026, 9, 28), Some(7));

        assert_eq!(removed, 2, "只应删 2020-01-01 与 2026-09-21");
        let left: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        for name in ["2026-09-22.log", "2026-09-28.log", "backup.log", "notes.txt", "2026-09-28.log.bak"] {
            assert!(left.contains(&name.to_string()), "{name} 不该被删：{left:?}");
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn cleanup_on_missing_dir_is_a_noop() {
        let root = temp_dir("missing");
        assert_eq!(cleanup(&log_dir(&root), d(2026, 9, 28), Some(7)), 0);
    }

    #[test]
    fn date_arithmetic_round_trips_across_boundaries() {
        // 自纪元天数与民用历互逆（含闰年、跨月、跨年）
        for days in [0i64, -1, 19_000, 20_654, 20_695, 21_000, 100_000] {
            let (y, m, dd) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, dd), days, "{days} 往返不一致");
        }
        // 闰年 2 月 29 日确实存在，且比 3 月 1 日早一天
        assert_eq!(
            days_from_civil(2024, 3, 1) - days_from_civil(2024, 2, 29),
            1
        );
    }

    #[test]
    fn write_creates_dir_and_file_readable_immediately() {
        let root = temp_dir("write");
        let dir = log_dir(&root);
        let sink = LogSink::open_in(dir.clone(), Some(7));

        let at = LocalTime {
            date: d(2026, 9, 28),
            hour: 8,
            minute: 0,
            second: 1,
            millis: 5,
        };
        sink.write_at(at, LEVEL_INFO, "app", "启动");

        let path = log_path_in(&dir, d(2026, 9, 28));
        assert!(dir.is_dir(), "目录应被自动创建");
        let text = read(&path);
        assert_eq!(text, "2026-09-28 08:00:01.005 [INFO] [app] 启动\n");

        // 换到第二天：新文件收到新行，旧文件不再增长
        let next = LocalTime {
            date: d(2026, 9, 29),
            hour: 0,
            minute: 0,
            second: 2,
            millis: 1,
        };
        sink.write_at(next, LEVEL_WARN, "app", "跨日");
        assert_eq!(read(&path), "2026-09-28 08:00:01.005 [INFO] [app] 启动\n");
        assert_eq!(
            read(&log_path_in(&dir, d(2026, 9, 29))),
            "2026-09-29 00:00:02.001 [WARN] [app] 跨日\n"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn write_failure_is_silent_and_retried_later() {
        let root = temp_dir("blocked");
        fs::create_dir_all(&root).unwrap();
        // 用同名文件占住 `logs` 这个位置，使建目录失败
        fs::write(root.join(LOG_DIR_NAME), b"not a dir").unwrap();
        let sink = LogSink::open_in(log_dir(&root), Some(7));
        let at = LocalTime {
            date: d(2026, 9, 28),
            hour: 8,
            minute: 0,
            second: 1,
            millis: 5,
        };
        // 不 panic、不报错（这一行就是写不出去）
        sink.write_at(at, LEVEL_ERROR, "app", "写不出去");

        // 障碍移除后，后续写入照常成功，不必重启
        fs::remove_file(root.join(LOG_DIR_NAME)).unwrap();
        sink.write_at(at, LEVEL_INFO, "app", "恢复");
        let text = read(&log_path_in(&log_dir(&root), d(2026, 9, 28)));
        assert_eq!(text, "2026-09-28 08:00:01.005 [INFO] [app] 恢复\n");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn set_retention_cleans_immediately() {
        let root = temp_dir("retention");
        let dir = log_dir(&root);
        fs::create_dir_all(&dir).unwrap();
        let old = dir.join("2020-01-01.log");
        fs::write(&old, b"x").unwrap();

        let sink = LogSink::open_in(dir.clone(), None); // 永久：不删
        sink.cleanup_now();
        assert!(old.exists(), "永久保留不该删任何文件");

        sink.set_retention(Some(7)); // 改成 7 天：立即清理
        assert!(!old.exists(), "改策略后应立即按新策略清理");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn uninitialized_sink_is_a_noop() {
        // 单例未初始化时调用不得 panic（也不能写入任何地方）。
        // 本测试刻意不调用 `init`：进程内单例只此一份，任何 init 都会影响同进程的其它测试。
        info("test", "未初始化时静默丢弃");
        warn("test", "同上");
        error("test", "同上");
    }

    // 非 Windows 的取时路径只在 Linux 上编译：编译检查只看得到签名，看不出字段映射
    // 与毫秒换算，这一条是它们唯一的运行时把关（也正因如此，它在 Windows 上不参与编译）。
    #[cfg(not(windows))]
    #[test]
    fn local_now_fields_are_in_range() {
        let now = local_now();
        assert!(now.date.year >= 2020, "年份不合理：{}", now.date.year);
        assert!((1..=12).contains(&now.date.month), "月份越界：{}", now.date.month);
        assert!((1..=31).contains(&now.date.day), "日期越界：{}", now.date.day);
        assert!(now.hour < 24, "小时越界：{}", now.hour);
        assert!(now.minute < 60, "分钟越界：{}", now.minute);
        assert!(now.second < 60, "秒越界：{}", now.second);
        assert!(now.millis < 1000, "毫秒越界：{}", now.millis);
    }
}
