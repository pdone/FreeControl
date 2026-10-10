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
//! - **探测层** —— 后台线程每 [`POLL_INTERVAL`] 轮询一次 `adb devices`，得到「设备是否存在」；
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
//! 最近一次 adb 探测结果存放在 `AppState.adb_probe`（而不是只留在轮询线程的局部变量里），
//! 这样连接 / 断开之后可以**立即**重算并推送状态，不必干等下一次轮询。

use crate::assets::ADB_BIN;
use crate::config::{self, AppConfig, Device};
use crate::logger;
use crate::scrcpy;
use crate::session;
use crate::AppState;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
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
fn mutate_and_persist<F>(
    app: &AppHandle,
    state: &State<'_, AppState>,
    mutate: F,
) -> Result<AppConfig, String>
where
    F: FnOnce(&mut AppConfig) -> Result<(), String>,
{
    let mut candidate = state.config.lock().unwrap().clone();
    mutate(&mut candidate)?;
    config::save_config_to_disk(app, &candidate)?;
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
) -> Result<AppConfig, String> {
    device.id.clear();
    mutate_and_persist(&app, &state, |cfg| {
        let id = cfg.upsert_device(device)?;
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
) -> Result<AppConfig, String> {
    mutate_and_persist(&app, &state, |cfg| {
        let id = cfg.upsert_device(device)?;
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
) -> Result<AppConfig, String> {
    session::stop_and_forget(&app, &id)?;
    let cfg = mutate_and_persist(&app, &state, |cfg| {
        // 先取快照再删：删掉之后配置里已经没有它了，而日志要记的正是刚刚删掉的那一条。
        let snapshot = cfg.devices.iter().find(|d| d.id == id).cloned();
        if cfg.remove_device(&id) {
            if let Some(d) = snapshot.as_ref() {
                log_device("删除", d);
            }
            Ok(())
        } else {
            Err("设备不存在".to_string())
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

/// 轮询间隔（design D13：秒级折中值）。`adb devices` 只是查询已启动的 adb 服务端，
/// 单次开销很小；2 秒既能让插拔 / 断网及时反映，也不会无谓地拉起子进程。
pub const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// 单次 `adb devices` 的超时。超时即本次探测失败，**不改变已有状态**
/// （design D13 风险条目：一次探测被拖慢不能把在用设备误判为离线）。
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
/// 放在 `AppState` 里（而不是只留在轮询线程的局部变量）是为了让**其它线程**也能取到当前探测结果：
/// 连接 / 断开设备后要立即重算状态并推送，否则界面要干等最长一个轮询周期才反映出来。
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
    /// 最近一次探测失败的原因（成功时为 `None`）
    pub error: Option<String>,
    /// 后端当前持有的设备列表（可能含轮询期间补全的 serial；前端据此同步）
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
    output
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
pub fn adopt_unknown_serial(cfg: &mut AppConfig, adb: &HashSet<String>) -> Option<String> {
    let claimed: HashSet<String> = cfg
        .devices
        .iter()
        .map(|d| d.connection_id())
        .filter(|s| !s.is_empty())
        .collect();
    let unclaimed: Vec<&String> = adb.iter().filter(|s| !claimed.contains(*s)).collect();
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

// ── 探测（子进程） ─────────────────────────────────────────

/// 执行命令并收集标准输出；带超时，超时会杀掉子进程并返回错误。
///
/// 不用 `Command::output()`：它没有超时，adb 服务端异常时会把轮询线程永远挂住。
/// 本函数只在轮询线程里被调用（轮询命令串行化），故不会并发抢占。
/// `session` 里的 `taskkill`（优雅结束会话）复用它，同样不能在无超时的情况下阻塞。
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

/// 轮询一次 `adb devices`：`Ok(集合)` 表示本次探测有效（集合可能为空 —— 有效但无设备），
/// `Err` 表示本次探测不可用（缺 adb / 超时 / 返回非零），调用方据此沿用上次结果。
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

/// 立刻重新探测一次并把结果写进状态。
///
/// 用于建立链路之后：`publish_status` 读的是**存量**探测结果，不重跑 adb，
/// 所以不先刷新的话界面要等下一轮轮询（最长 2 秒）才反映出来（design D2）。
pub fn refresh_probe(app: &AppHandle) {
    let probe = match scrcpy::scrcpy_dir(app) {
        Ok(dir) => probe_adb(&dir),
        Err(e) => Err(e),
    };
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
    DeviceStatusPayload {
        statuses,
        // 只有「沿用了上次结果」才算待确认；从未探测成功时 probed=false，前端显示「检测中」。
        stale: probe.error.is_some() && probe.devices.is_some(),
        probed: probe.devices.is_some(),
        error: probe.error,
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
/// 会话状态变化（连接 / 断开 / 启动失败 / 异常退出）不属于「探测到的新事实」，光靠轮询
/// 最迟要一个轮询周期才反映出来，故这些动作结束后主动调一次本函数，让界面即时刷新。
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
/// 下一轮轮询会再试。
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

/// 启动设备状态轮询线程（tasks 6.3）。
///
/// 用独立线程而非异步任务：整个循环是阻塞式的（子进程 + `sleep`），放进线程池只会
/// 白占一个异步工作线程。前端不自行探测，只消费这里推来的事件。
pub fn spawn_status_poller(app: AppHandle) {
    std::thread::spawn(move || loop {
        // 先回收已退出的会话（tasks 9.3）：进程退出可能是用户主动断开，也可能是异常退出，
        // 无论哪种都要让设备状态与槽位占用立刻跟上。
        if session::reap_exited(&app) {
            let infos = session::session_infos(&app.state::<AppState>());
            session::publish_sessions(&app, &infos);
        }
        // 安装（含替换）进行期间整体跳过探测（design D3）。
        match probe_step(install_in_flight(&app)) {
            ProbeStep::Skip => {}
            ProbeStep::Probe => {
                let probe = match scrcpy::scrcpy_dir(&app) {
                    Ok(dir) => probe_adb(&dir),
                    Err(e) => Err(e),
                };
                store_probe(&app, probe);
                let adb = probe_snapshot(&app.state::<AppState>()).devices;
                if let Some(adb) = adb.as_ref() {
                    persist_adopted_serial(&app, adb);
                }
                publish_status(&app);
            }
        }
        std::thread::sleep(POLL_INTERVAL);
    });
}

/// 一次轮询中设备状态那部分该做什么（纯函数）：安装进行期间整体跳过。
///
/// 跳过的两个后果都是有意的：既不发起 adb 调用（否则会把替换前刚结束的 adb 服务端
/// 在 2 秒内重新拉起，替换再次失败），也不写入新的探测结果 —— 界面沿用上次成功结果，
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
        // 慢命令必须被超时截断（并杀掉子进程），否则轮询线程会被永远挂住。
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
        // 安装（含替换）期间必须整体跳过探测：探测会把替换前刚结束的 adb 服务端在 2 秒内
        // 重新拉起，让替换再次失败在占用检查上。极性写反是静默失效 ——
        // 只有真机上替换失败时才会发现，故在此锁死。
        assert_eq!(probe_step(true), ProbeStep::Skip);
        assert_eq!(probe_step(false), ProbeStep::Probe);
    }

    #[test]
    fn link_connect_needs_the_binary_and_keeps_a_short_budget() {
        let err = connect_link(Path::new(r"C:\definitely\not\here"), "192.168.1.5:5555").unwrap_err();
        assert!(err.contains(ADB_BIN));

        // 预算必须短于探测：连接不可达地址会阻塞到 TCP 超时，等太久界面看起来像卡死。
        assert!(LINK_TIMEOUT < PROBE_TIMEOUT);
    }
}