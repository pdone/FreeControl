//! 设备管理。
//!
//! ## Group 5：设备列表的读写持久化（tasks 5.3）
//! 「别名 ⇄ 连接标识」的映射与增删改落盘在这一层；标识的算法来源是
//! `config::Device::connection_id`（USB 用 serial，无线用 `IP:端口`），本模块不重复实现。
//!
//! 三个写命令都**返回落盘后的完整配置**，而不是只回传设备列表：删除设备会连带清除其参数
//! 覆盖与覆盖开关，若只回传设备列表，前端那份 `config` 会残留已删设备的覆盖，
//! 随后的去抖保存又会把它写回盘上。
//!
//! ## Group 6：实时状态（tasks 6.1–6.5，design D13）
//! 状态由后端统一判定后下发，前端只渲染：
//! - **探测层** —— 后台线程订阅 adb 服务端的设备变化通知（`host:track-devices`），得到
//!   「设备是否存在」；断线时沿用上次结果、按有界节奏重连，空闲时不发起任何探测；
//! - **会话层** —— `AppState.sessions` 保存本应用持有活跃会话的设备 id（进程管理在 Group 9 接入）；
//! - 合并规则见 [`merge_statuses`]：**会话表优先于探测结果**；
//! - 结果只在变化时以事件 [`EVENT_DEVICE_STATUS`] 推给前端，同时缓存一份供
//!   `get_device_status` 取快照 —— 否则「首次探测早于前端注册监听」会让界面永远停在检测中；
//! - 单次探测失败**不改变已有状态**：沿用上一次成功结果并标记 `stale`（design D13 风险条目）；
//! - 另在探测成功后按 1:1 规则补全「serial 未知的 USB 设备」的连接标识（spec 场景「添加 USB 设备」）。
//!
//! ## Group 9：会话层接入（tasks 9.1–9.3）
//! 会话表由 [`crate::session`] 持有；本模块只从它取「**仍在运行**的会话」，于是
//! 「单个会话异常退出 → 该设备回到空闲（界面上的「在线」）、其它会话不受影响」是自然结果：某台设备的记录一旦被
//! 标记为已退出，[`merge_statuses`] 就不再把它算作已连接，落回探测结果。
//!
//! 最近一次 adb 探测结果存放在 `AppState.adb_probe`（而不是只留在订阅线程的局部变量里），
//! 这样连接 / 断开之后可以**立即**重算并推送状态，不必干等下一次通知。

use crate::assets::ADB_BIN;
use crate::config::{self, AppConfig, Device};
use crate::logger;
use crate::scrcpy;
use crate::session;
use crate::ui_message::{codes, UserError};
use crate::AppState;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub fn get_devices(state: State<'_, AppState>) -> Vec<Device> {
    state.config.lock().unwrap().devices.clone()
}

/// 在一次「读-改-写」事务里修改配置并立即落盘，返回落盘后的完整配置。
///
/// 修改先作用于副本：校验或落盘失败时内存与磁盘都保持原样，不会出现
/// 「界面报错、内存却已经改了」这种半生效状态（重启后又变回去）。
///
/// `pub(crate)`：无线配对 / 发现（`crate::wireless`）登记设备时复用同一套事务语义，
/// 不另写一份「先落盘再改内存」的顺序。
pub(crate) fn mutate_and_persist<F>(
    app: &AppHandle,
    state: &State<'_, AppState>,
    mutate: F,
) -> Result<AppConfig, UserError>
where
    F: FnOnce(&mut AppConfig) -> Result<(), UserError>,
{
    let mut candidate = state.config.lock().unwrap().clone();
    mutate(&mut candidate)?;
    config::save_config_to_disk(app, &candidate)
        .map_err(|e| UserError::new(codes::CONFIG_SAVE_FAILED).with_detail(e))?;
    *state.config.lock().unwrap() = candidate.clone();
    Ok(candidate)
}

/// 记一行设备变更（新增 / 修改 / 删除）：取值取自**规范化之后**的登记项，与落盘内容一致。
fn log_device(action: &str, d: &Device) {
    logger::info(
        "device",
        format!(
            "{action}设备「{}」（连接标识 {}，id {}）",
            d.alias,
            d.connection_id(),
            d.id
        ),
    );
}

/// 新增设备（tasks 5.3 / 6.1）：id 由后端分配，前端不需要自己造 id。
#[tauri::command]
pub fn add_device(
    app: AppHandle,
    state: State<'_, AppState>,
    mut device: Device,
) -> Result<AppConfig, UserError> {
    device.id.clear();
    mutate_and_persist(&app, &state, |cfg| {
        let id = cfg
            .upsert_device(device)
            .map_err(|e| UserError::new(codes::DEVICE_INVALID).with_detail(e))?;
        // 用 upsert 分配好的 id 回查：日志里的连接标识与落盘的那一条逐字一致
        // （直接打印入参会在无线设备未填端口时记下一个 `:0` 的假标识）。
        if let Some(d) = cfg.devices.iter().find(|d| d.id == id) {
            log_device("新增", d);
        }
        Ok(())
    })
}

/// 更新设备：别名内联编辑、探测到 serial 后补全连接标识都走这里（spec「编辑别名」）。
#[tauri::command]
pub fn update_device(
    app: AppHandle,
    state: State<'_, AppState>,
    device: Device,
) -> Result<AppConfig, UserError> {
    mutate_and_persist(&app, &state, |cfg| {
        let id = cfg
            .upsert_device(device)
            .map_err(|e| UserError::new(codes::DEVICE_INVALID).with_detail(e))?;
        if let Some(d) = cfg.devices.iter().find(|d| d.id == id) {
            log_device("修改", d);
        }
        Ok(())
    })
}

/// 删除设备：别名映射、参数覆盖与覆盖启停开关一并清除（device-management spec）。
///
/// 若该设备仍有会话在运行，先结束它（含槽位与会话记录的清理）：设备记录没了、进程还活着，
/// 会留下一个无人管理、也无人能断开的投屏窗口。
#[tauri::command]
pub fn delete_device(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<AppConfig, UserError> {
    session::stop_and_forget(&app, &id)
        .map_err(|e| UserError::new(codes::SESSION_STOP_FAILED).with_detail(e))?;
    let cfg = mutate_and_persist(&app, &state, |cfg| {
        // 先取快照再删：删掉之后配置里已经没有它了，而日志要记的正是刚刚删掉的那一条。
        let snapshot = cfg.devices.iter().find(|d| d.id == id).cloned();
        if cfg.remove_device(&id) {
            if let Some(d) = snapshot.as_ref() {
                log_device("删除", d);
            }
            Ok(())
        } else {
            Err(UserError::new(codes::DEVICE_NOT_FOUND))
        }
    })?;
    publish_status(&app);
    Ok(cfg)
}

// ── 运行期状态（tasks 6.3–6.4，design D13） ──────────────────

/// 已连接：本应用持有的会话表中存在该设备。
pub const STATUS_CONNECTED: &str = "connected";
/// 空闲（界面上的「在线」）：设备在 `adb devices` 中可见，且本应用无该设备的会话。
pub const STATUS_IDLE: &str = "idle";
/// 离线：`adb devices` 中不存在该设备（或状态不是 `device`）。
pub const STATUS_OFFLINE: &str = "offline";

/// 状态推送事件名（前端 `listen` 订阅）。
pub const EVENT_DEVICE_STATUS: &str = "device-status";

/// adb 服务端的缺省端点（回环 + 官方默认端口）。
pub const DEFAULT_SERVER_ENDPOINT: &str = "127.0.0.1:5037";

/// 订阅线程的读超时：超时分支承担会话回收、安装窗口与退避检查。
///
/// 它**不**表示「设备状态过期」：`host:track-devices` 长时间无帧是正常状态
/// （设备列表没变就不推），空闲超时不能当作连接已死。
const SUBSCRIBE_TICK: Duration = Duration::from_millis(500);

/// 等待服务端按帧响应的预算（`host:version` / `host:devices`）：走本地回环，正常毫秒级
/// 返回；给足预算只为兜住「5037 被别的程序占着却不说话」。
const SERVER_REPLY_TIMEOUT: Duration = Duration::from_secs(5);

/// 服务端协议版本下限：能按帧回答 `host:version` 且不低于它，才认作可用的 adb 服务端。
const MIN_SERVER_PROTOCOL: u32 = 0x001f;

/// 单帧净荷上限：设备列表只有几十行；超过它说明读到的不是 adb 帧。
///
/// 帧长度前缀是 4 位十六进制（最大 65535），这里取更小的 32 KiB —— 正常净荷不足 1 KiB。
const MAX_FRAME_LEN: usize = 32 * 1024;

/// 断线后的重连退避（取到末项即封顶）。
const RECONNECT_BACKOFF: [Duration; 5] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(10),
    Duration::from_secs(30),
];

/// 拉起服务端之后的连接重试间隔：`adb start-server` 返回时 fork 出的服务端可能还没开始
/// 监听，立刻重连会失败并撞进下一档退避 —— 白等几秒。累计 750ms 足够覆盖这段时间。
const STARTUP_RETRY: [Duration; 4] = [
    Duration::from_millis(50),
    Duration::from_millis(100),
    Duration::from_millis(200),
    Duration::from_millis(400),
];

/// 单次 `adb` 子进程调用的超时（`start-server`、诊断用 `adb devices`）。超时即本次调用
/// 失败、**不改变已有状态**（design D13 风险条目：一次调用被拖慢不能把在用设备误判为离线）。
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// 建立无线链路的超时预算。
///
/// 比探测短一档（design D3）：`adb connect` 对不可达地址会一直阻塞到 TCP 超时，而这条
/// 命令本身没有可配的超时参数，只能靠外部终止子进程。设备在线时正常毫秒级返回。
pub const LINK_TIMEOUT: Duration = Duration::from_secs(5);

/// 等待子进程退出的轮询步长。
const WAIT_TICK: Duration = Duration::from_millis(30);

/// 最近一次 adb 探测的结果。
///
/// 放在 `AppState` 里（而不是只留在订阅线程的局部变量）是为了让**其它线程**也能取到当前探测结果：
/// 连接 / 断开设备后要立即重算状态并推送，否则界面要干等下一次设备变化通知才反映出来。
/// 探测失败时 `devices` 保持上一次成功的结果（design D13 风险条目：一次探测被拖慢不能把在用设备
/// 误判为离线），失败原因另记在 `error` 里。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AdbProbe {
    pub devices: Option<HashSet<String>>,
    pub error: Option<String>,
}

/// 下发给前端的设备状态快照（事件与命令共用同一结构）。
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatusPayload {
    /// 设备 id → `connected` / `idle` / `offline`
    pub statuses: HashMap<String, String>,
    /// 最近一次探测失败、当前结果沿用上一次成功探测（界面标注「状态待确认」）
    pub stale: bool,
    /// 是否至少成功探测过一次（前端据此区分「检测中」与真正的「离线」）
    pub probed: bool,
    /// 最近一次探测失败的原因（成功时为 `None`）；文案由前端按 code 出
    pub error: Option<UserError>,
    /// 后端当前持有的设备列表（可能含订阅期间补全的 serial；前端据此同步）
    pub devices: Vec<Device>,
}

// ── 纯函数：adb 输出解析 / 状态合并 / serial 补全 ─────────────

/// 解析 `adb devices` 的标准输出，返回状态为 `device` 的连接标识集合。
///
/// 输出形如（platform-tools 的 `adb devices` 实现）：
///
/// ```text
/// List of devices attached
/// 08081FDD4006LYE	device
/// 192.168.1.5:5555	device
/// emulator-5554	offline
/// ```
///
/// 只把 `device` 视为「设备存在」；`offline` / `unauthorized` / `no permissions` 都不算 ——
/// 它们无法发起会话，在界面上就是「离线」。表头（`List of devices attached`）与
/// 守护进程提示（`* daemon ...`）的第二个字段都不是 `device`，天然被滤掉。
pub fn parse_adb_devices(output: &str) -> HashSet<String> {
    parse_track_devices(output)
}

/// 解析设备列表净荷：`adb devices` 的标准输出与 `host:track-devices` 的帧净荷同构
/// （后者没有 `List of devices attached` 表头，逐行解析天然兼容）。
///
/// 只把 `device` 视为「设备存在」；`offline` / `unauthorized` / `no permissions` 都不算 ——
/// 它们无法发起会话，在界面上就是「离线」。
pub fn parse_track_devices(payload: &str) -> HashSet<String> {
    payload
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let serial = parts.next()?;
            let state = parts.next()?;
            if state == "device" {
                Some(serial.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// 合并「配置中的设备」「adb 探测结果」「会话表」得出每台设备的状态（design D13）。
///
/// `sessions` 只包含**仍在运行**的会话（见 [`crate::session::active_ids`]）。
///
/// - **已连接**：会话表中存在该设备且进程仍在运行 —— 该判断**优先于探测结果**：
///   会话刚启动时 adb 可能尚未反映；反过来，会话一旦结束就绝不残留「已连接」
///   （哪怕 adb 仍然列着这台设备）。
/// - **空闲**（界面上的「在线」）：会话表没有，但 `adb devices` 中存在其连接标识。
/// - **离线**：其余情况（含「连接标识未知」与「本次探测不可用」）。
pub fn merge_statuses(
    devices: &[Device],
    adb: Option<&HashSet<String>>,
    sessions: &HashSet<String>,
) -> HashMap<String, String> {
    devices
        .iter()
        .map(|d| {
            let status = if sessions.contains(&d.id) {
                STATUS_CONNECTED
            } else {
                let conn = d.connection_id();
                match adb {
                    Some(set) if !conn.is_empty() && set.contains(&conn) => STATUS_IDLE,
                    _ => STATUS_OFFLINE,
                }
            };
            (d.id.clone(), status.to_string())
        })
        .collect()
}

/// 把 adb 中唯一一条未被登记的连接标识，补到唯一一台 serial 未知的 USB 设备上
/// （device-management spec 场景「添加 USB 设备」：serial 留空 → 后续检测到再补全）。
///
/// 只在「恰好一台待补全设备 + 恰好一条未认领标识」时补全：这是唯一能 1:1 对应的情形，
/// 多对多时任何自动指派都可能张冠李戴 —— 宁可留空由用户自己填。
/// 已在别处登记过的标识不会被抢走，否则会凭空改掉另一台设备的连接标识。
///
/// **mDNS 服务名不算候选**（[`is_mdns_service_name`]）：已配对的无线设备会被 adb 自动连出一条
/// 以服务名命名的 transport，它与用户显式 `adb connect` 出来的 `IP:端口` 指向同一台设备。
/// 不排除它，一台 serial 留空的 USB 设备就可能被写上 `adb-xxx._adb-tls-connect._tcp`。
pub fn adopt_unknown_serial(cfg: &mut AppConfig, adb: &HashSet<String>) -> Option<String> {
    let claimed: HashSet<String> = cfg
        .devices
        .iter()
        .map(|d| d.connection_id())
        .filter(|s| !s.is_empty())
        .collect();
    let unclaimed: Vec<&String> = adb
        .iter()
        .filter(|s| !claimed.contains(*s))
        .filter(|s| !is_mdns_service_name(s))
        .collect();
    if unclaimed.len() != 1 {
        return None;
    }
    let unknown: Vec<usize> = cfg
        .devices
        .iter()
        .enumerate()
        .filter(|(_, d)| d.kind == config::KIND_USB && d.connection_id().is_empty())
        .map(|(i, _)| i)
        .collect();
    if unknown.len() != 1 {
        return None;
    }
    let serial = unclaimed[0].clone();
    cfg.devices[unknown[0]].serial = serial.clone();
    Some(serial)
}

/// 形如 `adb-5c17e2d3-URzewu._adb-tls-connect._tcp` 的标识：adb 通过 mDNS **自动**连接
/// 已配对无线设备时产生的 transport 名，2026-10-23 实测与 `IP:端口` 并存于同一条设备。
///
/// 它不是任何 USB 设备的 serial（USB 侧永远是设备硬件序列号），故只在「补全 serial」这一处
/// 被排除；`merge_statuses` 仍按 `IP:端口` 匹配，不受这条 transport 影响。
pub fn is_mdns_service_name(serial: &str) -> bool {
    serial.contains("._adb-tls-") || serial.ends_with("._adb._tcp")
}

// ── 探测（子进程） ─────────────────────────────────────────

/// 执行命令并收集标准输出；带超时，超时会杀掉子进程并返回错误。
///
/// 不用 `Command::output()`：它没有超时，adb 服务端异常时会把调用线程永远挂住。
/// 调用点分散在订阅线程（`start-server`）、会话层（`adb connect` / `taskkill`）与诊断路径；
/// 各自的调用串行、不共享可变状态，故不会并发抢占。
///
/// **只适用于输出很小的命令**：实现是「轮询等退出 → 退出后才读管道」，命令若写出超过管道缓冲
/// （Windows 上数十 KB）就会阻塞在写、直到本函数的超时被杀掉。需要看大输出时（如
/// `dumpsys display`）请在**设备侧**先过滤（见 `session::query_display_size`）。
pub(crate) fn run_captured(bin: &Path, args: &[&str], timeout: Duration) -> Result<String, String> {
    let mut cmd = Command::new(bin);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        // 探测子进程不得弹出控制台窗口。
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("探测超时（超过 {} 秒）", timeout.as_secs()));
                }
                std::thread::sleep(WAIT_TICK);
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() {
            format!("{} 退出码 {}", bin.display(), out.status)
        } else {
            err
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// 一次性调用 `adb devices`：`Ok(集合)` 表示本次有效（集合可能为空 —— 有效但无设备），
/// `Err` 表示本次不可用（缺 adb / 超时 / 返回非零），调用方据此沿用上次结果。
///
/// 设备状态的常规来源已改为订阅（`host:track-devices`）；这条路径只在服务端拉不起来时
/// 作为复核与诊断保留 —— 它也是订阅出问题时的回滚出口。
fn probe_adb(dir: &Path) -> Result<HashSet<String>, String> {
    let adb = dir.join(ADB_BIN);
    if !adb.is_file() {
        return Err(format!(
            "运行环境内未找到 {}，设备状态暂不可用",
            ADB_BIN
        ));
    }
    let out = run_captured(&adb, &["devices"], PROBE_TIMEOUT)?;
    Ok(parse_adb_devices(&out))
}

/// 为无线设备建立 adb 链路；返回 adb 的**原始输出**。
///
/// 返回 `Ok` 只表示命令跑完了，**不代表链路已建立**：`adb connect` 连不上时也可能以 0 退出，
/// 失败原因写在输出里（如 `failed to connect to <地址>: Connection refused`）。成功与否由调用方
/// 按重探测结果判定（见 `session::ensure_wireless_link` 与 design D2）；原文原样带回去，
/// 让界面与日志都能给出可分辨的原因，而不是一句「连接失败」。
pub fn connect_link(dir: &Path, connection_id: &str) -> Result<String, String> {
    let adb = dir.join(ADB_BIN);
    if !adb.is_file() {
        return Err(format!("运行环境内未找到 {ADB_BIN}，无法建立无线链路"));
    }
    run_captured(&adb, &["connect", connection_id], LINK_TIMEOUT)
}

/// 立刻查询一次设备列表并把结果写进状态。
///
/// 用于建立链路之后：`publish_status` 读的是**存量**探测结果，不重跑查询，
/// 所以不先刷新的话界面要等订阅的下一帧才反映出来（design D2）。
///
/// 走 `host:devices` 短连接而不是再起一次 `adb devices` —— 后者会创建一个子进程，
/// 而「空闲/操作中都不为设备状态起进程」正是本变更要保住的性质。
pub fn refresh_probe(app: &AppHandle) {
    let probe = query_devices_now();
    store_probe(app, probe);
}

// ── 状态快照与推送 ─────────────────────────────────────────

/// 最近一次 adb 探测结果（各自短暂加锁，不嵌套）。
pub fn probe_snapshot(state: &AppState) -> AdbProbe {
    state.adb_probe.lock().unwrap().clone()
}

/// 配置中的设备（各自短暂加锁，不嵌套）。
fn devices_snapshot(state: &AppState) -> Vec<Device> {
    state.config.lock().unwrap().devices.clone()
}

/// 采集合成一份状态快照：配置中的设备 + 会话表 + 最近一次 adb 探测结果。
///
/// 三次取数各自加锁后立即释放，不做嵌套加锁 —— 锁顺序一旦在某条路径上反转就会死锁。
pub fn build_payload(app: &AppHandle) -> DeviceStatusPayload {
    let state = app.state::<AppState>();
    let devices = devices_snapshot(&state);
    let probe = probe_snapshot(&state);
    let sessions = session::active_ids(&state);
    let statuses = merge_statuses(&devices, probe.devices.as_ref(), &sessions);
    // 只有「沿用了上次结果」才算待确认；从未探测成功时 probed=false，前端显示「检测中」。
    let stale = probe.error.is_some() && probe.devices.is_some();
    let probed = probe.devices.is_some();
    DeviceStatusPayload {
        statuses,
        stale,
        probed,
        error: probe
            .error
            .map(|e| UserError::new(codes::DEVICE_STATUS_PROBE_FAILED).with_detail(e)),
        devices,
    }
}

/// 某台设备当前的运行期状态；设备不存在时为 `None`。
///
/// 供会话层判定「能否连接」——**状态判定只有这一份实现**，不在会话层另写一套规则。
pub fn status_of_device(state: &AppState, device_id: &str) -> Option<String> {
    let devices = devices_snapshot(state);
    if !devices.iter().any(|d| d.id == device_id) {
        return None;
    }
    let probe = probe_snapshot(state);
    let sessions = session::active_ids(state);
    merge_statuses(&devices, probe.devices.as_ref(), &sessions)
        .get(device_id)
        .cloned()
}

/// 重算并在**变化时**推送状态（tasks 6.3）。
///
/// 会话状态变化（连接 / 断开 / 启动失败 / 异常退出）不属于「探测到的新事实」，光靠订阅
/// 要等下一次设备变化通知才反映出来，故这些动作结束后主动调一次本函数，让界面即时刷新。
/// 变化判定以 `AppState.device_status`（最近一次下发的那份）为准，避免重复推送。
pub fn publish_status(app: &AppHandle) {
    let payload = build_payload(app);
    let state = app.state::<AppState>();
    {
        let mut cache = state.device_status.lock().unwrap();
        if cache.as_ref() == Some(&payload) {
            return;
        }
        *cache = Some(payload.clone());
    }
    // 推送放在锁外：事件可能引发前端继续发命令，拿着锁推送等于给自己埋一个死锁。
    let _ = app.emit(EVENT_DEVICE_STATUS, payload);
}

/// 把探测到的 serial 补进配置并落盘。落盘失败则不改内存（与设备写命令同一语义），
/// 下一帧设备变化通知到达时会再试。
fn persist_adopted_serial(app: &AppHandle, adb: &HashSet<String>) {
    let state = app.state::<AppState>();
    let mut candidate = state.config.lock().unwrap().clone();
    let Some(serial) = adopt_unknown_serial(&mut candidate, adb) else {
        return;
    };
    if config::save_config_to_disk(app, &candidate).is_err() {
        return;
    }
    // 这也是一次「用户没操作、盘上却变了」的配置写入，记一行解释它的来由。
    if let Some(d) = candidate.devices.iter().find(|d| d.serial == serial) {
        logger::info(
            "device",
            format!("探测补全连接标识：「{}」→ {serial}", d.alias),
        );
    }
    *state.config.lock().unwrap() = candidate;
}

/// 记录本次探测结果：成功时覆盖设备集合并清掉错误，失败时**保留上次成功的集合**、只记原因。
///
/// 失败与恢复**只在状态翻转时**记一行：轮询每 2 秒一轮，不判翻转就会把日志刷满，
/// 而真正要排查的正是「什么时候开始失败、什么时候恢复的」这两条时间点。
fn store_probe(app: &AppHandle, result: Result<HashSet<String>, String>) {
    let state = app.state::<AppState>();
    let mut probe = state.adb_probe.lock().unwrap();
    match result {
        Ok(set) => {
            if let Some(prev) = probe.error.take() {
                logger::info(
                    "device",
                    format!("连接标识探测已恢复（此前失败原因：{prev}）"),
                );
            }
            probe.devices = Some(set);
        }
        Err(e) => {
            if probe.error.as_deref() != Some(e.as_str()) {
                logger::warn(
                    "device",
                    format!("连接标识探测失败：{e}（沿用上一次成功的结果）"),
                );
            }
            probe.error = Some(e);
        }
    }
}

/// 启动设备状态订阅线程（`host:track-devices`）。
///
/// 用独立线程而非异步任务：整条循环是阻塞式的（socket 读），放进线程池只会白占一个
/// 异步工作线程。前端不自行探测，只消费这里推来的事件。
pub fn spawn_status_subscriber(app: AppHandle) {
    std::thread::spawn(move || {
        let mut backoff = 0usize;
        let mut next_attempt = Instant::now();
        loop {
            // 安装（含替换）窗口内整体暂停：不连接、也不拉起服务端 —— 否则会把替换前
            // 刚结束的服务端重新拉起，替换再次失败在占用检查上。
            if probe_step(install_in_flight(&app)) == ProbeStep::Skip {
                std::thread::sleep(SUBSCRIBE_TICK);
                backoff = 0;
                next_attempt = Instant::now();
                continue;
            }
            if Instant::now() < next_attempt {
                housekeeping(&app);
                std::thread::sleep(SUBSCRIBE_TICK);
                continue;
            }
            match track_session(&app) {
                SessionEnd::Paused => backoff = 0,
                SessionEnd::Failed(e) => {
                    // 失败原因进快照：既有语义是「沿用上次成功的集合 + 标记待确认」，
                    // 且只在状态翻转时记一行（`store_probe` 负责），这里不重复记。
                    store_probe(&app, Err(e));
                    publish_status(&app);
                    next_attempt = Instant::now() + reconnect_delay(backoff);
                    backoff = (backoff + 1).min(RECONNECT_BACKOFF.len() - 1);
                }
            }
        }
    });
}

/// 第 `attempt` 次重连前的等待时长（末项封顶）。
fn reconnect_delay(attempt: usize) -> Duration {
    RECONNECT_BACKOFF[attempt.min(RECONNECT_BACKOFF.len() - 1)]
}

/// 一次订阅会话的结束方式。
///
/// 安装窗口内的退出是**主动让位**，不写探测结果：暂停不是失败，界面沿用上次成功结果，
/// `stale` 也不置位（与 [`ProbeStep::Skip`] 的语义一致）。
enum SessionEnd {
    Paused,
    Failed(String),
}

/// 建立订阅并一直消费到断开。
fn track_session(app: &AppHandle) -> SessionEnd {
    // 校验用独立连接：adb 的 host 服务一条连接只服务一个请求，`host:version` 应答后
    // 连接即被服务端关闭，不能拿它继续发订阅请求。
    if let Err(e) = verify_server(app) {
        return if install_in_flight(app) {
            SessionEnd::Paused
        } else {
            SessionEnd::Failed(e)
        };
    }
    let mut stream = match connect_server(app) {
        Ok(stream) => stream,
        Err(e) => return SessionEnd::Failed(e),
    };
    if let Err(e) = stream.set_read_timeout(Some(SUBSCRIBE_TICK)) {
        return SessionEnd::Failed(format!("设置读超时失败：{e}"));
    }
    if let Err(e) = stream.write_all(&encode_frame("host:track-devices")) {
        return SessionEnd::Failed(format!("订阅设备变化失败：{e}"));
    }
    // 帧流前面还有一次应答状态码；漏读会把它当成帧长度。
    if let Err(e) = read_status(&mut stream, SERVER_REPLY_TIMEOUT) {
        return if install_in_flight(app) {
            SessionEnd::Paused
        } else {
            SessionEnd::Failed(e)
        };
    }

    consume_track_stream(
        &mut stream,
        &mut |payload| apply_track_frame(app, payload),
        &mut || {
            housekeeping(app);
            !install_in_flight(app)
        },
    )
}

/// 消费一条订阅连接直到断开或让位。抽成独立函数是为了能用本地 `TcpListener` 桩验证
/// 「首帧到达即回调」「半帧跨读超时不丢」「读超时只做杂务」这几件容易出错的事。
///
/// `on_tick` 在读超时时调用，返回 `false` 表示让位（安装窗口），此时连接由调用方丢弃。
fn consume_track_stream(
    stream: &mut impl Read,
    on_frame: &mut dyn FnMut(&str),
    on_tick: &mut dyn FnMut() -> bool,
) -> SessionEnd {
    let mut reader = FrameReader::default();
    let mut buf = [0u8; 4096];
    loop {
        match stream.read(&mut buf) {
            // 对端关闭：安装窗口内的关闭由替换路径结束服务端所致，不算失败。
            Ok(0) => {
                if !on_tick() {
                    return SessionEnd::Paused;
                }
                return SessionEnd::Failed(
                    reader
                        .broken_tail()
                        .unwrap_or_else(|| "adb 服务端关闭了订阅连接".to_string()),
                );
            }
            Ok(n) => {
                reader.push(&buf[..n]);
                loop {
                    match reader.next_frame() {
                        Ok(Some(frame)) => on_frame(&frame),
                        Ok(None) => break,
                        Err(e) => return SessionEnd::Failed(e),
                    }
                }
            }
            // 空闲是常态（设备没变就不推），超时只用来做与「设备变化」无关的周期杂务。
            Err(e) if is_read_timeout(&e) => {
                if !on_tick() {
                    return SessionEnd::Paused;
                }
            }
            Err(e) => return SessionEnd::Failed(format!("读取设备变化通知失败：{e}")),
        }
    }
}

/// 一帧设备列表到达：写快照 → 补 serial → 推送。与既有轮询的一轮等价。
fn apply_track_frame(app: &AppHandle, payload: &str) {
    let devices = parse_track_devices(payload);
    store_probe(app, Ok(devices));
    let adb = probe_snapshot(&app.state::<AppState>()).devices;
    if let Some(adb) = adb.as_ref() {
        persist_adopted_serial(app, adb);
    }
    publish_status(app);
}

/// 周期杂务：回收已退出的会话（tasks 9.3）。
///
/// 有变化时除同步会话列表外，还必须重算设备状态：会话退出改变「已连接 / 在线」的判定，
/// 而订阅只在**设备列表**变化时推帧 —— 不在这里补一次推送，界面会停在「已连接」直到下次插拔。
fn housekeeping(app: &AppHandle) {
    if session::reap_exited(app) {
        let infos = session::session_infos(&app.state::<AppState>());
        session::publish_sessions(app, &infos);
        publish_status(app);
    }
}

/// 建立到 adb 服务端的连接；连不上时先拉起一次服务端（首启，或它被结束后）。
pub(crate) fn connect_server(app: &AppHandle) -> Result<TcpStream, String> {
    let endpoint = server_endpoint();
    if let Ok(stream) = TcpStream::connect(endpoint.as_str()) {
        return Ok(stream);
    }
    let dir = scrcpy::scrcpy_dir(app)?;
    ensure_server_running(&dir)?;
    let mut last = String::new();
    for wait in STARTUP_RETRY {
        std::thread::sleep(wait);
        match TcpStream::connect(endpoint.as_str()) {
            Ok(stream) => return Ok(stream),
            Err(e) => last = e.to_string(),
        }
    }
    Err(format!("无法连接 adb 服务端（{endpoint}）：{last}"))
}

/// 拉起 adb 服务端：**唯一**会为设备状态创建子进程的路径，且只在连接失败时发生。
fn ensure_server_running(dir: &Path) -> Result<(), String> {
    let adb = dir.join(ADB_BIN);
    if !adb.is_file() {
        return Err(format!("运行环境内未找到 {}，设备状态暂不可用", ADB_BIN));
    }
    match run_captured(&adb, &["start-server"], PROBE_TIMEOUT) {
        Ok(_) => Ok(()),
        // `start-server` 失败时用 `adb devices` 再确认一次：它同样会拉起服务端，
        // 且错误更贴近真实原因（缺 DLL、端口被占用这类）。
        Err(e) => probe_adb(dir)
            .map(|_| ())
            .map_err(|fallback| format!("{e}（改用 adb devices 亦失败：{fallback}）")),
    }
}

// ── adb host 协议（帧编解码 / 端点解析） ──────────────────────

/// 端点解析（纯函数）：`ADB_SERVER_SOCKET` 优先，其次地址 + 端口，缺省回环 5037。
///
/// 三条来源与 adb 客户端读的是同一组环境变量 —— 若这里与子进程路径各自解析，
/// 订阅连上的服务端与 `adb connect` 用的就会是两个。
fn server_endpoint_from(
    socket: Option<&str>,
    address: Option<&str>,
    port: Option<&str>,
) -> String {
    if let Some(rest) = socket.map(str::trim).and_then(|s| s.strip_prefix("tcp:")) {
        if is_host_port(rest) {
            return rest.to_string();
        }
    }
    let host = address
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("127.0.0.1");
    let port = match port.map(str::trim).filter(|s| !s.is_empty()) {
        None => "5037".to_string(),
        Some(p) if is_port(p) => p.to_string(),
        // 非法端口整条退化为缺省：宁可连不上、按「待确认」降级，也不要连到一个猜出来的端点。
        Some(_) => return DEFAULT_SERVER_ENDPOINT.to_string(),
    };
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// 进程环境中的端点（读取与 adb 客户端同一组变量）。
pub(crate) fn server_endpoint() -> String {
    let socket = std::env::var("ADB_SERVER_SOCKET").ok();
    let address = std::env::var("ANDROID_ADB_SERVER_ADDRESS").ok();
    let port = std::env::var("ANDROID_ADB_SERVER_PORT").ok();
    server_endpoint_from(socket.as_deref(), address.as_deref(), port.as_deref())
}

fn is_port(s: &str) -> bool {
    s.parse::<u16>().is_ok()
}

/// `host:port` 形态：端口必须是合法端口，主机段非空（IPv6 的 `[::1]:5037` 也算）。
fn is_host_port(s: &str) -> bool {
    match s.rsplit_once(':') {
        Some((host, port)) => !host.is_empty() && is_port(port),
        None => false,
    }
}

/// 按 adb 的 host 协议编码一次请求：4 位十六进制长度 + 净荷。
pub(crate) fn encode_frame(payload: &str) -> Vec<u8> {
    let mut out = format!("{:04x}", payload.len()).into_bytes();
    out.extend_from_slice(payload.as_bytes());
    out
}

/// 增量帧解码器：一次读可能只带回半帧，也可能一次带回多帧。
#[derive(Default)]
struct FrameReader {
    buf: Vec<u8>,
}

impl FrameReader {
    fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// 取下一帧；长度前缀还不完整时返回 `None`（等下一段数据）。
    ///
    /// 长度前缀非法时清空缓冲并返回错误 —— 连接已被污染，调用方应重连而不是续读。
    fn next_frame(&mut self) -> Result<Option<String>, String> {
        if self.buf.len() < 4 {
            return Ok(None);
        }
        let len = match std::str::from_utf8(&self.buf[..4])
            .ok()
            .and_then(|s| usize::from_str_radix(s, 16).ok())
        {
            Some(len) if len <= MAX_FRAME_LEN => len,
            _ => {
                self.buf.clear();
                return Err("adb 服务端返回了无法解析的帧长度".to_string());
            }
        };
        if self.buf.len() < 4 + len {
            return Ok(None);
        }
        let payload = String::from_utf8_lossy(&self.buf[4..4 + len]).into_owned();
        self.buf.drain(..4 + len);
        Ok(Some(payload))
    }

    /// 读到对端关闭：缓冲里还留着半帧，说明这次连接不可信。
    fn broken_tail(&mut self) -> Option<String> {
        if self.buf.is_empty() {
            return None;
        }
        let n = self.buf.len();
        self.buf.clear();
        Some(format!("连接在帧中途结束（已读 {n} 字节）"))
    }
}

fn is_read_timeout(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
    )
}

/// 按预算读满 `out`：读超时只是续读，不算失败（超时用来兜住「对端不说话」）。
fn read_exact_with_budget(
    stream: &mut TcpStream,
    out: &mut [u8],
    budget: Duration,
) -> Result<(), String> {
    let deadline = Instant::now() + budget;
    let mut filled = 0usize;
    while filled < out.len() {
        if Instant::now() >= deadline {
            return Err("等待 adb 服务端响应超时".to_string());
        }
        match stream.read(&mut out[filled..]) {
            Ok(0) => return Err("adb 服务端在响应前关闭了连接".to_string()),
            Ok(n) => filled += n,
            Err(e) if is_read_timeout(&e) => continue,
            Err(e) => return Err(format!("读取 adb 服务端响应失败：{e}")),
        }
    }
    Ok(())
}

/// 读一次请求的应答状态：adb 对每个 service 请求先回 4 字节 `OKAY` 或 `FAIL`。
///
/// 这一层不是可选的：帧流前面就是这个状态码，漏读会把 `OKAY` 当成帧长度。
/// `FAIL` 后面跟一段长度前缀的错误原文，原样带回给调用方（界面与日志都要能读出原因）。
pub(crate) fn read_status(stream: &mut TcpStream, budget: Duration) -> Result<(), String> {
    let mut code = [0u8; 4];
    read_exact_with_budget(stream, &mut code, budget)?;
    match &code {
        b"OKAY" => Ok(()),
        b"FAIL" => {
            let reason =
                read_one_frame(stream, budget).unwrap_or_else(|_| "（未附带原因）".to_string());
            Err(format!("adb 服务端拒绝了请求：{reason}"))
        }
        _ => Err(format!(
            "adb 服务端返回了无法识别的应答：{:?}",
            String::from_utf8_lossy(&code)
        )),
    }
}

/// 等待下一个完整帧（一次性请求用）；超出预算或对端关闭即失败。
pub(crate) fn read_one_frame(stream: &mut TcpStream, budget: Duration) -> Result<String, String> {
    let deadline = Instant::now() + budget;
    let mut reader = FrameReader::default();
    let mut buf = [0u8; 4096];
    loop {
        if let Some(frame) = reader.next_frame()? {
            return Ok(frame);
        }
        if Instant::now() >= deadline {
            return Err("等待 adb 服务端响应超时".to_string());
        }
        match stream.read(&mut buf) {
            Ok(0) => {
                return Err(reader
                    .broken_tail()
                    .unwrap_or_else(|| "adb 服务端在响应前关闭了连接".to_string()))
            }
            Ok(n) => reader.push(&buf[..n]),
            Err(e) if is_read_timeout(&e) => continue,
            Err(e) => return Err(format!("读取 adb 服务端响应失败：{e}")),
        }
    }
}

/// 建一条独立连接做协议校验（连不上时按 [`connect_server`] 的规则拉起服务端）。
fn verify_server(app: &AppHandle) -> Result<(), String> {
    let mut stream = connect_server(app)?;
    stream
        .set_read_timeout(Some(SUBSCRIBE_TICK))
        .map_err(|e| format!("设置读超时失败：{e}"))?;
    verify_protocol_on(&mut stream)
}

/// 确认对端确实是一个可用的 adb 服务端（在一条**尚未发过请求**的连接上调用）。
///
/// 5037 可能被别的程序占用（或服务端过旧）：此时按帧请求只会得到垃圾或超时。
/// 这里只判「能不能按帧回答一个足够新的协议版本」，**不**做客户端与服务端的版本对齐 ——
/// 那需要结束服务端，而它可能属于同机的其它工具。
fn verify_protocol_on(stream: &mut TcpStream) -> Result<(), String> {
    stream
        .write_all(&encode_frame("host:version"))
        .map_err(|e| format!("向 adb 服务端发送请求失败：{e}"))?;
    read_status(stream, SERVER_REPLY_TIMEOUT)?;
    let payload = read_one_frame(stream, SERVER_REPLY_TIMEOUT)?;
    let version = u32::from_str_radix(payload.trim(), 16)
        .map_err(|_| format!("adb 服务端返回的协议版本无法解析：{payload:?}"))?;
    if version < MIN_SERVER_PROTOCOL {
        return Err(format!("adb 服务端协议版本过旧（{version:04x}）"));
    }
    Ok(())
}

/// 一次性查询当前设备列表（`host:devices`）。
///
/// 供「无线链路建立后本次操作内即刷新」使用：与订阅走同一套帧协议，因此**不创建子进程**。
pub fn query_devices_now() -> Result<HashSet<String>, String> {
    query_devices_at(server_endpoint().as_str())
}

/// 查询实际设备列表；查询失败时先拉起 adb 服务端再试一次（只在这一条异常路径创建子进程）。
///
/// 为什么需要重试：运行环境刚被替换时服务端已被结束、而界面状态仍是替换前的「在线」——
/// 这正是最需要复核的窗口。若此时直接按「查询失败」回退，复核在最关键的时刻失效。
pub fn live_devices(app: &AppHandle) -> Result<HashSet<String>, String> {
    let first = match query_devices_now() {
        Ok(set) => return Ok(set),
        Err(e) => e,
    };
    let dir = scrcpy::scrcpy_dir(app)?;
    ensure_server_running(&dir)?;
    query_devices_now().map_err(|e| format!("{first}；拉起服务端后仍失败：{e}"))
}

/// 一次性查询（可指定端点，便于用本地桩验证）。
fn query_devices_at(endpoint: &str) -> Result<HashSet<String>, String> {
    let mut stream =
        TcpStream::connect(endpoint).map_err(|e| format!("无法连接 adb 服务端（{endpoint}）：{e}"))?;
    stream
        .set_read_timeout(Some(SUBSCRIBE_TICK))
        .map_err(|e| format!("设置读超时失败：{e}"))?;
    stream
        .write_all(&encode_frame("host:devices"))
        .map_err(|e| format!("向 adb 服务端发送请求失败：{e}"))?;
    read_status(&mut stream, SERVER_REPLY_TIMEOUT)?;
    let payload = read_one_frame(&mut stream, SERVER_REPLY_TIMEOUT)?;
    Ok(parse_track_devices(&payload))
}

/// 向 adb 服务端发一条 **host 服务**请求并取回一帧净荷（`host:mdns:services` 这类）。
///
/// 与 [`query_devices_now`] 同一套帧协议、同一个端点解析，因此同样**不创建子进程**；
/// 连不上时按 [`connect_server`] 的规则拉起服务端（首启，或它被结束后）。
///
/// 服务名不在此处校验：`FAIL` 的原文（如 `unknown host service 'mdns:services'`）
/// 原样带回给调用方 —— 「adb 不支持这项服务」与「服务端出问题」的处置完全不同。
pub(crate) fn query_host(app: &AppHandle, service: &str) -> Result<String, String> {
    let mut stream = connect_server(app)?;
    query_host_on(&mut stream, service)
}

/// 在一条**尚未发过请求**的连接上发一次 host 服务请求并读回一帧。
fn query_host_on(stream: &mut TcpStream, service: &str) -> Result<String, String> {
    stream
        .set_read_timeout(Some(SUBSCRIBE_TICK))
        .map_err(|e| format!("设置读超时失败：{e}"))?;
    stream
        .write_all(&encode_frame(service))
        .map_err(|e| format!("向 adb 服务端发送请求失败：{e}"))?;
    read_status(stream, SERVER_REPLY_TIMEOUT)?;
    read_one_frame(stream, SERVER_REPLY_TIMEOUT)
}

/// 一次设备状态判定该做什么（纯函数）：安装进行期间整体跳过。
///
/// 跳过的两个后果都是有意的：既不发起连接（否则会把替换前刚结束的 adb 服务端在
/// 下一轮重连里重新拉起，替换再次失败），也不写入新的探测结果 —— 界面沿用上次成功结果，
/// 而 `stale` 不会被置位（它只在探测失败时置位，暂停不算失败）。
#[derive(Debug, PartialEq, Eq)]
enum ProbeStep {
    Probe,
    Skip,
}

fn probe_step(install_in_flight: bool) -> ProbeStep {
    if install_in_flight {
        ProbeStep::Skip
    } else {
        ProbeStep::Probe
    }
}

/// 是否有在途安装（含替换阶段）。判据用现成的安装生命周期，不另设标志位。
fn install_in_flight(app: &AppHandle) -> bool {
    app.state::<AppState>().install.lock().unwrap().is_some()
}

/// 设备状态快照（tasks 6.3）。
///
/// 事件是主通道，但事件只在**变化时**推送：若首次推送早于前端注册监听（启动竞态），
/// 界面会一直停在「检测中」。故提供一个可随时取用的快照，前端启动时先取一次。
#[tauri::command]
pub fn get_device_status(app: AppHandle) -> DeviceStatusPayload {
    build_payload(&app)
}

#[cfg(test)]
mod tests {
    //! 状态判定的可验证部分（tasks 6.1 / 6.3 / 6.4）：
    //! 解析、合并、serial 补全都是纯函数，可直接构造输入验证；子进程部分只验证
    //! 「带超时地执行命令」这一层（用系统自带命令，不依赖真实 adb 与设备）。

    use super::*;
    use std::net::TcpListener;

    // 只有 Windows 那几个用 `cmd.exe` 的用例用到它，故随用例一起门控；
    // 不加这层条件的话，Linux 侧编译（那些用例不参与）会报一条 unused import。
    #[cfg(windows)]
    use std::path::PathBuf;

    fn usb(id: &str, alias: &str, serial: &str) -> Device {
        Device {
            id: id.to_string(),
            alias: alias.to_string(),
            kind: config::KIND_USB.to_string(),
            serial: serial.to_string(),
            ip: String::new(),
            port: 0,
        }
    }

    fn wifi(id: &str, alias: &str, ip: &str, port: u16) -> Device {
        Device {
            id: id.to_string(),
            alias: alias.to_string(),
            kind: config::KIND_WIFI.to_string(),
            serial: String::new(),
            ip: ip.to_string(),
            port,
        }
    }

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    // ── 6.3：adb 输出解析 ─────────────────────────────────────

    #[test]
    fn parses_adb_devices_output() {
        // 真实输出形态：表头 + 制表符分隔，混有 offline / unauthorized。
        let out = "List of devices attached\r\n\
                   08081FDD4006LYE\tdevice\r\n\
                   192.168.1.5:5555\tdevice\r\n\
                   emulator-5554\toffline\r\n\
                   ABC123\tunauthorized\r\n\
                   \r\n";
        let got = parse_adb_devices(out);

        assert_eq!(got.len(), 2);
        assert!(got.contains("08081FDD4006LYE"));
        assert!(got.contains("192.168.1.5:5555"));
        // 只有 device 才算「设备存在」：offline / unauthorized 都在界面上呈现为离线。
        assert!(!got.contains("emulator-5554"));
        assert!(!got.contains("ABC123"));
    }

    #[test]
    fn parses_empty_and_noisy_output() {
        // 没有设备时只有表头（adb 自身启动提示走 stderr，但多解析几个词也不该误判成设备）。
        assert!(parse_adb_devices("List of devices attached\n\n").is_empty());
        assert!(parse_adb_devices("").is_empty());
        assert!(parse_adb_devices("* daemon not running; starting now at tcp:5037\n").is_empty());
    }

    // ── 6.3：状态合并（会话表优先） ────────────────────────────

    #[test]
    fn merge_covers_three_states() {
        let devices = vec![
            usb("dev-1", "手机", "S1"),
            wifi("dev-2", "平板", "192.168.1.5", 5555),
            usb("dev-3", "备用机", "S3"),
        ];
        // dev-3 不在 adb 列表里 → 离线。
        let adb = set(&["S1", "192.168.1.5:5555"]);
        let sessions = set(&["dev-1"]);

        let got = merge_statuses(&devices, Some(&adb), &sessions);

        assert_eq!(got.len(), 3);
        assert_eq!(got["dev-1"], STATUS_CONNECTED);
        assert_eq!(got["dev-2"], STATUS_IDLE);
        assert_eq!(got["dev-3"], STATUS_OFFLINE);
    }

    #[test]
    fn session_table_wins_over_adb_probe() {
        // 会话刚启动时 adb 可能尚未反映 —— 仍算已连接（会话表优先）。
        let devices = vec![usb("dev-1", "手机", "S1")];
        let got = merge_statuses(&devices, Some(&set(&[])), &set(&["dev-1"]));
        assert_eq!(got["dev-1"], STATUS_CONNECTED);
    }

    #[test]
    fn ended_session_is_not_connected_even_if_adb_still_lists_it() {
        // 会话已结束：即使 adb 仍列着这台设备，也只能算空闲（界面上的「在线」）——不能残留「已连接」。
        let devices = vec![usb("dev-1", "手机", "S1")];
        let got = merge_statuses(&devices, Some(&set(&["S1"])), &set(&[]));
        assert_eq!(got["dev-1"], STATUS_IDLE);
    }

    #[test]
    fn unknown_identifier_or_unavailable_probe_is_offline() {
        // serial 未知的 USB 设备：无法与 adb 列表对应，判离线（界面上另标「待检测」）。
        let devices = vec![usb("dev-1", "新设备", "")];
        let got = merge_statuses(&devices, Some(&set(&["S1"])), &set(&[]));
        assert_eq!(got["dev-1"], STATUS_OFFLINE);

        // 探测不可用（缺 adb / 超时）：同样不谎报可用。
        let got = merge_statuses(&devices, None, &set(&[]));
        assert_eq!(got["dev-1"], STATUS_OFFLINE);
    }

    // ── 6.1：serial 补全（spec 场景「添加 USB 设备」） ─────────

    #[test]
    fn adopts_serial_when_exactly_one_candidate() {
        let mut cfg = AppConfig::fresh();
        let id = cfg.upsert_device(usb("", "新设备", "")).unwrap();

        let adopted = adopt_unknown_serial(&mut cfg, &set(&["S-9"]));

        assert_eq!(adopted.as_deref(), Some("S-9"));
        assert_eq!(cfg.devices[0].id, id, "补全 serial 不应换 id");
        assert_eq!(cfg.devices[0].connection_id(), "S-9");
    }

    #[test]
    fn does_not_adopt_when_candidates_are_ambiguous() {
        // 两条未认领标识 → 无法 1:1 对应，留空由用户自己填。
        let mut cfg = AppConfig::fresh();
        cfg.upsert_device(usb("", "新设备", "")).unwrap();
        assert!(adopt_unknown_serial(&mut cfg, &set(&["S-1", "S-2"])).is_none());
        assert_eq!(cfg.devices[0].connection_id(), "");

        // 两台待补全设备 → 同样不猜。
        let mut cfg = AppConfig::fresh();
        cfg.upsert_device(usb("", "A", "")).unwrap();
        cfg.upsert_device(usb("", "B", "")).unwrap();
        assert!(adopt_unknown_serial(&mut cfg, &set(&["S-1"])).is_none());

        // 没有任何未认领标识（都已登记）→ 不补全，也不改动既有标识。
        let mut cfg = AppConfig::fresh();
        cfg.upsert_device(usb("", "已知", "S-1")).unwrap();
        cfg.upsert_device(usb("", "新设备", "")).unwrap();
        assert!(adopt_unknown_serial(&mut cfg, &set(&["S-1"])).is_none());
        assert_eq!(cfg.devices[0].connection_id(), "S-1");
        assert_eq!(cfg.devices[1].connection_id(), "");
    }

    #[test]
    fn does_not_adopt_for_wifi_records() {
        // 无线设备的标识是用户填的 IP:端口，不该被 adb 里的 serial 覆盖。
        let mut cfg = AppConfig::fresh();
        cfg.upsert_device(wifi("", "平板", "192.168.1.5", 5555)).unwrap();
        assert!(adopt_unknown_serial(&mut cfg, &set(&["192.168.1.5:5555"])).is_none());
    }

    #[test]
    fn does_not_adopt_an_mdns_service_name_for_a_usb_device() {
        // 2026-10-23 真机实测：已配对的无线设备会被 adb 自动连出一条**服务名** transport，
        // 它与显式 `adb connect` 出的 `IP:端口` 同时在 `adb devices` 里（同一台设备两条）。
        // 不排除它，一台 serial 留空的 USB 设备就会被写上这串服务名。
        let mut cfg = AppConfig::fresh();
        cfg.upsert_device(usb("", "新设备", "")).unwrap();

        assert!(adopt_unknown_serial(&mut cfg, &set(&["adb-abc._adb-tls-connect._tcp"])).is_none());
        assert_eq!(cfg.devices[0].connection_id(), "", "不该被写上服务名");

        // 同一次探测里既有真 serial 又有服务名：服务名不参与计数，真 serial 照旧补上。
        let mut cfg = AppConfig::fresh();
        cfg.upsert_device(usb("", "新设备", "")).unwrap();
        let adopted =
            adopt_unknown_serial(&mut cfg, &set(&["S-9", "adb-abc._adb-tls-connect._tcp"]));
        assert_eq!(adopted.as_deref(), Some("S-9"));
        assert_eq!(cfg.devices[0].connection_id(), "S-9");

        // 旧式 `_adb._tcp` 与服务名形态都要排除；真 serial 与无线标识不受影响。
        assert!(is_mdns_service_name("adb-x._adb._tcp"));
        assert!(is_mdns_service_name("adb-5c17e2d3-URzewu._adb-tls-connect._tcp"));
        assert!(!is_mdns_service_name("08081FDD4006LYE"));
        assert!(!is_mdns_service_name("192.168.1.5:5555"));
    }

    // ── 6.3：带超时的子进程执行 ───────────────────────────────

    #[test]
    #[cfg(windows)]
    fn runs_a_real_command_and_returns_stdout() {
        let out = run_captured(
            &PathBuf::from("cmd.exe"),
            &["/C", "echo", "List of devices attached"],
            Duration::from_secs(10),
        )
        .unwrap();
        assert!(out.contains("List of devices attached"));
    }

    #[test]
    #[cfg(windows)]
    fn parses_output_of_a_real_process() {
        // 端到端验证「执行进程 → 解析」这一段（用 echo 冒充一次 adb devices 的输出）。
        let dir = std::env::temp_dir();
        let script = dir.join(format!("fcn-adb-stub-{}.txt", std::process::id()));
        std::fs::write(
            &script,
            "List of devices attached\r\nSERIAL-1\tdevice\r\nSERIAL-2\tunauthorized\r\n",
        )
        .unwrap();

        let out = run_captured(
            &PathBuf::from("cmd.exe"),
            &["/C", "type", &script.to_string_lossy()],
            Duration::from_secs(10),
        )
        .unwrap();
        let got = parse_adb_devices(&out);

        let _ = std::fs::remove_file(&script);
        assert_eq!(got.len(), 1);
        assert!(got.contains("SERIAL-1"));
    }

    #[test]
    #[cfg(windows)]
    fn hangs_are_bounded_by_timeout() {
        // 慢命令必须被超时截断（并杀掉子进程），否则调用线程会被永远挂住。
        let started = Instant::now();
        let err = run_captured(
            &PathBuf::from("cmd.exe"),
            &["/C", "ping", "-n", "10", "127.0.0.1"],
            Duration::from_millis(300),
        )
        .unwrap_err();

        assert!(err.contains("超时"), "实际错误：{}", err);
        assert!(started.elapsed() < Duration::from_secs(8), "超时后应立即返回");
    }

    #[test]
    fn missing_binary_is_reported() {
        let err = probe_adb(Path::new(r"C:\definitely\not\here")).unwrap_err();
        assert!(err.contains(ADB_BIN));
    }

    #[test]
    fn install_in_flight_skips_probing() {
        // 安装（含替换）期间必须整体跳过探测：探测会把替换前刚结束的 adb 服务端在下一轮
        // 重连里重新拉起，让替换再次失败在占用检查上。极性写反是静默失效 ——
        // 只有真机上替换失败时才会发现，故在此锁死。
        assert_eq!(probe_step(true), ProbeStep::Skip);
        assert_eq!(probe_step(false), ProbeStep::Probe);
    }

    // ── 订阅（host:track-devices）与一次性查询 ────────────────

    /// 起一个本地桩「adb 服务端」：接受一条连接后交给 `script`。
    fn stub_server<F>(script: F) -> (String, std::thread::JoinHandle<()>)
    where
        F: FnOnce(TcpStream) + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            script(stream);
        });
        (addr.to_string(), handle)
    }

    /// 按真实服务端的应答形状回一段：先 `OKAY`，再一帧。
    ///
    /// 这个 4 字节状态码曾漏读，导致把 `OKAY` 当成帧长度 —— 桩必须照真实形状回，
    /// 否则单测会在「实现漏读状态码」时照样全绿。
    fn write_okay_frame(stream: &mut TcpStream, payload: &str) {
        stream.write_all(b"OKAY").unwrap();
        stream.write_all(&encode_frame(payload)).unwrap();
    }

    /// 连上桩、发出订阅请求并消费应答状态码（与 `track_session` 的建连部分同形）。
    fn subscribe_to(endpoint: &str) -> TcpStream {
        let mut stream = TcpStream::connect(endpoint).unwrap();
        stream.set_read_timeout(Some(SUBSCRIBE_TICK)).unwrap();
        stream
            .write_all(&encode_frame("host:track-devices"))
            .unwrap();
        read_status(&mut stream, SERVER_REPLY_TIMEOUT).unwrap();
        stream
    }

    #[test]
    fn server_endpoint_follows_the_client_environment() {
        // 与 adb 客户端读同一组变量：`ADB_SERVER_SOCKET` 优先，其次地址 + 端口。
        assert_eq!(
            server_endpoint_from(Some("tcp:127.0.0.1:6000"), Some("10.0.0.1"), Some("5000")),
            "127.0.0.1:6000"
        );
        assert_eq!(
            server_endpoint_from(None, Some("10.0.0.1"), Some("5000")),
            "10.0.0.1:5000"
        );
        // 只给了端口：主机仍取缺省回环，不能整条退化成默认端点。
        assert_eq!(
            server_endpoint_from(None, None, Some("6001")),
            "127.0.0.1:6001"
        );
        assert_eq!(
            server_endpoint_from(None, None, None),
            DEFAULT_SERVER_ENDPOINT
        );
        // IPv6 主机要加方括号，否则拼出来的不是合法端点。
        assert_eq!(
            server_endpoint_from(None, Some("::1"), Some("5037")),
            "[::1]:5037"
        );
        // 空值 / 非法值一律退化为缺省，且不 panic。
        assert_eq!(
            server_endpoint_from(Some("tcp:"), None, None),
            DEFAULT_SERVER_ENDPOINT
        );
        assert_eq!(
            server_endpoint_from(Some("local:/tmp/adb"), None, None),
            DEFAULT_SERVER_ENDPOINT
        );
        assert_eq!(
            server_endpoint_from(None, Some("  "), Some("70000")),
            DEFAULT_SERVER_ENDPOINT
        );
        assert_eq!(
            server_endpoint_from(None, None, Some("abc")),
            DEFAULT_SERVER_ENDPOINT
        );
    }

    #[test]
    fn frame_reader_handles_split_and_batched_frames() {
        let payload = "S1\tdevice\n";
        let mut reader = FrameReader::default();

        // 一次投递整帧
        reader.push(&encode_frame(payload));
        assert_eq!(reader.next_frame().unwrap().as_deref(), Some(payload));
        assert!(reader.next_frame().unwrap().is_none());

        // 逐字节投递（net 读最碎的情形）
        for b in encode_frame(payload) {
            reader.push(&[b]);
        }
        assert_eq!(reader.next_frame().unwrap().as_deref(), Some(payload));

        // 一次投递多帧：两帧都要按顺序取出
        let mut two = encode_frame("A\tdevice\n");
        two.extend(encode_frame("B\toffline\n"));
        reader.push(&two);
        assert_eq!(reader.next_frame().unwrap().as_deref(), Some("A\tdevice\n"));
        assert_eq!(
            reader.next_frame().unwrap().as_deref(),
            Some("B\toffline\n")
        );
        assert!(reader.next_frame().unwrap().is_none());
    }

    #[test]
    fn frame_reader_rejects_bad_length_prefix() {
        // 非十六进制
        let mut reader = FrameReader::default();
        reader.push(b"zzzz");
        assert!(reader.next_frame().is_err());

        // 超过净荷上限（0xffff 已大于 32 KiB）：说明读到的不是 adb 帧
        let mut reader = FrameReader::default();
        reader.push(b"ffff");
        assert!(reader.next_frame().is_err());

        // 出错后缓冲被清空，不会在坏前缀上死循环
        assert_eq!(reader.next_frame().unwrap(), None);
    }

    #[test]
    fn frame_reader_reports_broken_tail_on_eof() {
        let mut reader = FrameReader::default();
        reader.push(&encode_frame("S1\tdevice\n")[..6]);
        assert!(reader.next_frame().unwrap().is_none(), "半帧还不该出帧");
        assert!(reader.broken_tail().is_some(), "EOF 时必须报出半帧");

        // 干净的 EOF 不算异常
        let mut reader = FrameReader::default();
        assert!(reader.broken_tail().is_none());
    }

    #[test]
    fn parses_track_payload_without_header() {
        // 订阅帧没有 `List of devices attached` 表头；解析结果必须与带表头时一致。
        let payload = "S1\tdevice\n192.168.1.5:5555\tdevice\nEMU\toffline\n";
        assert_eq!(
            parse_track_devices(payload),
            parse_adb_devices(&format!("List of devices attached\n{payload}"))
        );
        assert_eq!(
            parse_track_devices(payload),
            set(&["S1", "192.168.1.5:5555"])
        );
        assert!(parse_track_devices("").is_empty());
    }

    #[test]
    fn reconnect_delay_backs_off_and_caps() {
        let delays: Vec<Duration> = (0..8).map(reconnect_delay).collect();
        assert_eq!(delays[0], Duration::from_secs(1));
        assert_eq!(delays[1], Duration::from_secs(2));
        assert_eq!(delays[2], Duration::from_secs(5));
        assert_eq!(delays[3], Duration::from_secs(10));
        // 封顶后不再增长：否则「服务端长期不在」会变成越来越长的盲区。
        for w in delays.windows(2) {
            assert!(w[0] <= w[1]);
        }
        assert_eq!(delays[7], Duration::from_secs(30));
    }

    #[test]
    fn one_shot_query_reads_a_single_frame() {
        let (endpoint, handle) = stub_server(|mut stream| {
            let mut buf = [0u8; 64];
            let n = stream.read(&mut buf).unwrap();
            assert_eq!(&buf[..n], &encode_frame("host:devices")[..]);
            write_okay_frame(&mut stream, "S1\tdevice\n");
        });

        assert_eq!(query_devices_at(&endpoint).unwrap(), set(&["S1"]));
        handle.join().unwrap();
    }

    #[test]
    fn one_shot_query_reports_unreachable_server() {
        // 取一个刚释放的端口：连接必然被拒（不碰本机真实的 adb 服务端）。
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        drop(listener);

        let err = query_devices_at(&endpoint).unwrap_err();
        assert!(err.contains("无法连接"), "实际错误：{err}");
    }

    #[test]
    fn host_service_query_reads_a_single_frame() {
        // 与一次性设备查询同形，只换服务名：请求净荷逐字相等、返回净荷原样带回。
        let (endpoint, handle) = stub_server(|mut stream| {
            let mut buf = [0u8; 64];
            let n = stream.read(&mut buf).unwrap();
            assert_eq!(&buf[..n], &encode_frame("host:mdns:services")[..]);
            write_okay_frame(&mut stream, "List of discovered mdns services\n");
        });

        let mut stream = TcpStream::connect(&endpoint).unwrap();
        let got = query_host_on(&mut stream, "host:mdns:services").unwrap();

        assert_eq!(got, "List of discovered mdns services\n");
        handle.join().unwrap();
    }

    #[test]
    fn host_service_query_surfaces_fail_reason() {
        // adb 不支持某项 host 服务时回 `unknown host service`：原文必须带回，
        // 上层据此区分「adb 能力不足」与「服务端出问题」——两者给用户的处置完全不同。
        let (endpoint, handle) = stub_server(|mut stream| {
            let mut buf = [0u8; 64];
            let _ = stream.read(&mut buf);
            stream.write_all(b"FAIL").unwrap();
            stream.write_all(&encode_frame("unknown host service")).unwrap();
        });

        let mut stream = TcpStream::connect(&endpoint).unwrap();
        let err = query_host_on(&mut stream, "host:mdns:services").unwrap_err();

        assert!(err.contains("unknown host service"), "实际错误：{err}");
        handle.join().unwrap();
    }

    #[test]
    fn protocol_check_rejects_an_old_or_foreign_server() {
        // 协议版本过旧（0x0010 < 下限）：按订阅不可用降级，不结束服务端。
        let (endpoint, handle) = stub_server(|mut stream| {
            let mut buf = [0u8; 64];
            let _ = stream.read(&mut buf);
            write_okay_frame(&mut stream, "0010");
        });
        let mut stream = TcpStream::connect(&endpoint).unwrap();
        stream.set_read_timeout(Some(SUBSCRIBE_TICK)).unwrap();
        let err = verify_protocol_on(&mut stream).unwrap_err();
        assert!(err.contains("过旧"), "实际错误：{err}");
        handle.join().unwrap();

        // 5037 被别的程序占着：按帧请求得到的是垃圾，同样降级。
        let (endpoint, handle) = stub_server(|mut stream| {
            let mut buf = [0u8; 64];
            let _ = stream.read(&mut buf);
            stream.write_all(b"HTTP/1.1 200 OK\r\n").unwrap();
        });
        let mut stream = TcpStream::connect(&endpoint).unwrap();
        stream.set_read_timeout(Some(SUBSCRIBE_TICK)).unwrap();
        assert!(verify_protocol_on(&mut stream).is_err());
        handle.join().unwrap();

        // 正常协议版本（0x0029）：校验通过。
        let (endpoint, handle) = stub_server(|mut stream| {
            let mut buf = [0u8; 64];
            let _ = stream.read(&mut buf);
            write_okay_frame(&mut stream, "0029");
        });
        let mut stream = TcpStream::connect(&endpoint).unwrap();
        stream.set_read_timeout(Some(SUBSCRIBE_TICK)).unwrap();
        assert!(verify_protocol_on(&mut stream).is_ok());
        handle.join().unwrap();
    }

    #[test]
    fn status_reader_surfaces_fail_reasons() {
        // `FAIL` 后面跟长度前缀的错误原文：原样带回，界面与日志都要能读出原因。
        let (endpoint, handle) = stub_server(|mut stream| {
            // 这个用例只验状态码解析，不需要先收请求。
            stream.write_all(b"FAIL").unwrap();
            stream.write_all(&encode_frame("unknown host service")).unwrap();
        });
        let mut stream = TcpStream::connect(&endpoint).unwrap();
        stream.set_read_timeout(Some(SUBSCRIBE_TICK)).unwrap();
        let err = read_status(&mut stream, SERVER_REPLY_TIMEOUT).unwrap_err();
        assert!(err.contains("unknown host service"), "实际错误：{err}");
        handle.join().unwrap();
    }

    #[test]
    fn subscription_delivers_every_frame_in_order() {
        let (endpoint, handle) = stub_server(|mut stream| {
            let mut buf = [0u8; 64];
            let _ = stream.read(&mut buf);
            stream.write_all(b"OKAY").unwrap();
            stream.write_all(&encode_frame("S1\tdevice\n")).unwrap();
            stream.write_all(&encode_frame("S2\tdevice\n")).unwrap();
            // 让客户端先把两帧读走，再关闭。
            std::thread::sleep(Duration::from_millis(100));
        });

        let mut stream = subscribe_to(&endpoint);
        let mut frames: Vec<String> = Vec::new();
        let end = consume_track_stream(
            &mut stream,
            &mut |payload| frames.push(payload.to_string()),
            &mut || true,
        );

        assert!(
            matches!(end, SessionEnd::Failed(_)),
            "对端关闭应判为订阅失败（界面转「待确认」）"
        );
        assert_eq!(frames, vec!["S1\tdevice\n".to_string(), "S2\tdevice\n".to_string()]);
        handle.join().unwrap();
    }

    #[test]
    fn subscription_keeps_half_a_frame_across_read_timeouts() {
        let (endpoint, handle) = stub_server(|mut stream| {
            let mut buf = [0u8; 64];
            let _ = stream.read(&mut buf);
            stream.write_all(b"OKAY").unwrap();
            let frame = encode_frame("S1\tdevice\n");
            let (head, tail) = frame.split_at(3);
            stream.write_all(head).unwrap();
            // 超过一次读超时（SUBSCRIBE_TICK = 500ms）再把剩下的发过来。
            std::thread::sleep(Duration::from_millis(700));
            stream.write_all(tail).unwrap();
            std::thread::sleep(Duration::from_millis(50));
        });

        let mut stream = subscribe_to(&endpoint);
        let mut frames: Vec<String> = Vec::new();
        let mut ticks = 0usize;
        let _ = consume_track_stream(
            &mut stream,
            &mut |payload| frames.push(payload.to_string()),
            &mut || {
                ticks += 1;
                true
            },
        );

        assert_eq!(frames, vec!["S1\tdevice\n".to_string()]);
        assert!(ticks >= 1, "跨超时续读必须经历至少一次读超时");
        handle.join().unwrap();
    }

    #[test]
    fn subscription_yields_when_the_tick_asks_to_pause() {
        let (endpoint, handle) = stub_server(|mut stream| {
            let mut buf = [0u8; 64];
            let _ = stream.read(&mut buf);
            // 只给长度前缀、不给净荷：客户端会停在半帧上等，直到读超时。
            stream.write_all(b"OKAY").unwrap();
            stream.write_all(b"0009").unwrap();
            std::thread::sleep(Duration::from_millis(800));
        });

        let mut stream = subscribe_to(&endpoint);
        let end = consume_track_stream(&mut stream, &mut |_| {}, &mut || false);

        assert!(
            matches!(end, SessionEnd::Paused),
            "安装窗口内必须主动让位，而不是把半帧判成失败"
        );
        handle.join().unwrap();
    }

    #[test]
    fn link_connect_needs_the_binary_and_keeps_a_short_budget() {
        let err = connect_link(Path::new(r"C:\definitely\not\here"), "192.168.1.5:5555").unwrap_err();
        assert!(err.contains(ADB_BIN));

        // 预算必须短于探测：连接不可达地址会阻塞到 TCP 超时，等太久界面看起来像卡死。
        assert!(LINK_TIMEOUT < PROBE_TIMEOUT);
    }
}