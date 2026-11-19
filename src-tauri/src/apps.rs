//! 应用列表与应用打开。
//!
//! 应用清单来自运行环境内的 `scrcpy --list-apps`（来源 doc/scrcpy）：它与 `--start-app`
//! 使用同一套应用匹配口径，且不需要额外工具读取应用名称。scrcpy 在 list 模式下只打印结果
//! 后退出，不创建窗口；首次调用会推送 server 并扫描应用，可能接近 20 秒，故采集要并发读、
//! 超时给足，并避免在进程退出后才读管道造成死锁。
//!
//! 打开方式：
//! - 「新窗口打开」走 `session::start_session(..., allow_existing = true)`，用该设备生效参数
//!   再起一条 scrcpy，并临时追加 `--start-app=<包名>`；不写配置。该设备已有镜像主屏的会话时，
//!   本次改用独立虚拟显示器，见 `isolate_shared_main_display`。
//! - 「打开」（当前窗口）在该设备已有窗口时用 adb 在设备侧启动应用；没有窗口时按新窗口路径
//!   启动一条。虚拟显示器无法从外部定位，降级为新窗口；摄像头 / 无窗口会话改为设备侧启动
//!   并提示当前会话不显示设备屏幕。

use crate::assets::{self, ADB_BIN};
use crate::config::{Device, KIND_WIFI};
use crate::device;
use crate::download::run_blocking;
use crate::logger;
use crate::scrcpy;
use crate::session::{self, SessionInfo};
use crate::ui_message::{codes, UserError, UserMessage};
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

/// 首次列应用可能接近 20 秒（推 server + 设备扫描），留足余量。
const LIST_TIMEOUT: Duration = Duration::from_secs(60);
/// adb 的解析 / 启动单次预算。
const ADB_TIMEOUT: Duration = Duration::from_secs(15);
/// 采集上限：应用清单正常只有几十 KB；超过它说明读到的不是清单。
const MAX_CAPTURE: u64 = 4 * 1024 * 1024;
const WAIT_TICK: Duration = Duration::from_millis(30);

/// 一条可启动应用。`system` 保留 scrcpy 的 `*` / `-` 标记，界面目前只用于排序外的展示。
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    pub name: String,
    pub package: String,
    pub system: bool,
}

/// 打开应用的请求：参数由前端按该设备生效参数拼好；后端只注入一次性 `--start-app`。
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OpenAppRequest {
    pub device_id: String,
    pub package_name: String,
    /// `newWindow` / `currentWindow`
    pub mode: String,
    pub args: Vec<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OpenAppResult {
    /// 本次是否启动了新的 scrcpy 窗口（当前窗口打开可能只做设备侧启动）。
    pub started_window: bool,
    pub sessions: Vec<SessionInfo>,
    /// 会话已启动但最近启动时间未保存
    pub warning: Option<UserError>,
    /// 降级 / 提示（虚拟显示降级、摄像头或无窗口会话等）
    pub notice: Option<UserMessage>,
}

/// 读取设备上的可启动应用。
#[tauri::command]
pub async fn list_device_apps(app: AppHandle, device_id: String) -> Result<Vec<AppEntry>, UserError> {
    run_blocking(move || list_blocking(&app, &device_id)).await
}

/// 打开应用（新窗口 / 当前窗口）。
#[tauri::command]
pub async fn open_app(app: AppHandle, request: OpenAppRequest) -> Result<OpenAppResult, UserError> {
    run_blocking(move || open_blocking(&app, request)).await
}

fn list_blocking(app: &AppHandle, device_id: &str) -> Result<Vec<AppEntry>, UserError> {
    let (device, connection_id) = device_and_connection(app, device_id)?;
    let dir = environment_dir(app)?;
    let captured = capture_listing(&dir, &connection_id)
        .map_err(|e| UserError::new(codes::APPS_LIST_FAILED).with_detail(e))?;

    if is_unsupported_version(&captured.text) {
        return Err(UserError::new(codes::APPS_UNSUPPORTED_VERSION)
            .with_detail(tail_of(&captured.text)));
    }

    let entries = parse_app_list(&captured.text);
    if entries.is_empty() && (!captured.success || !captured.text.contains("List of apps:")) {
        let detail = tail_of(&captured.text);
        return Err(UserError::new(codes::APPS_LIST_FAILED).with_detail(if detail.is_empty() {
            "未读取到应用列表".to_string()
        } else {
            detail
        }));
    }

    logger::info(
        "apps",
        format!(
            "列出应用：{} 个（设备「{}」，连接标识 {}）",
            entries.len(),
            device.alias,
            connection_id
        ),
    );
    Ok(entries)
}

fn open_blocking(app: &AppHandle, request: OpenAppRequest) -> Result<OpenAppResult, UserError> {
    let package = request.package_name.trim().to_string();
    if !valid_package(&package) {
        return Err(UserError::new(codes::APPS_PACKAGE_INVALID));
    }
    let state = app.state::<AppState>();
    let device = session::find_device(&state, &request.device_id)?;

    match request.mode.as_str() {
        "currentWindow" => {
            let active = session::active_records(&state, &request.device_id);
            if !active.is_empty() {
                let choice = choose_display(&active);
                if choice == DisplayChoice::NewVirtual {
                    let args = with_start_app(request.args, &package);
                    let started = session::start_session(app, &device, args, true)?;
                    return finish(
                        app,
                        started,
                        true,
                        Some(UserMessage::new(codes::APPS_FALLBACK_NEW_WINDOW)),
                    );
                }
                let dir = environment_dir(app)?;
                let connection_id = active[0].connection_id.clone();
                let display = match choice {
                    DisplayChoice::Display(id) => Some(id),
                    _ => None,
                };
                launch_activity(&dir, &connection_id, &package, display)?;
                let notice = (choice == DisplayChoice::Headless)
                    .then(|| UserMessage::new(codes::APPS_HEADLESS_NOTICE));
                logger::info(
                    "apps",
                    format!(
                        "当前窗口打开「{}」（设备「{}」，连接标识 {}）",
                        package, device.alias, connection_id
                    ),
                );
                return Ok(OpenAppResult {
                    started_window: false,
                    sessions: session::session_infos(&state),
                    warning: None,
                    notice,
                });
            }
        }
        "newWindow" => {}
        other => {
            return Err(UserError::new(codes::APPS_OPEN_FAILED)
                .with_detail(format!("未知的打开方式：{other}")))
        }
    }

    if has_flag(&request.args, "--no-control") {
        return Err(UserError::new(codes::APPS_CONTROL_DISABLED));
    }
    let args = with_start_app(request.args, &package);
    let existing: Vec<Vec<String>> = session::active_records(&state, &request.device_id)
        .into_iter()
        .map(|r| r.args)
        .collect();
    let args = isolate_shared_main_display(args, &existing);
    let started = session::start_session(app, &device, args, true)?;
    finish(app, started, true, None)
}

/// 「新窗口打开」与已有窗口镜像同一块物理主屏时，本次改用独立虚拟显示器。
///
/// `--start-app` 会启动到本次镜像的显示上；若仍镜像主屏，设备屏幕会切到该应用，所有
/// 已开的主屏窗口一起跳屏。换成 `--new-display`（不带取值 = 主屏尺寸与密度）后应用只在
/// 新窗口里出现。同时剔除 `--no-vd-destroy-content`：保留它会在关闭窗口时把应用移到主屏，
/// 同样会把已有窗口拽走。
fn isolate_shared_main_display(args: Vec<String>, existing: &[Vec<String>]) -> Vec<String> {
    let shares_main = display_choice(&args) == DisplayChoice::Main
        && existing
            .iter()
            .any(|a| display_choice(a) == DisplayChoice::Main);
    if !shares_main {
        return args;
    }
    let mut out: Vec<String> = args
        .into_iter()
        .filter(|a| a != "--no-vd-destroy-content")
        .collect();
    out.push("--new-display".to_string());
    out
}

fn finish(
    app: &AppHandle,
    started: session::StartOutcome,
    started_window: bool,
    notice: Option<UserMessage>,
) -> Result<OpenAppResult, UserError> {
    let state = app.state::<AppState>();
    let sessions = session::session_infos(&state);
    session::publish_sessions(app, &sessions);
    device::publish_status(app);
    let notice = notice.or_else(|| {
        started
            .reused
            .then(|| UserMessage::new(codes::SESSION_WINDOW_ARRANGEMENT_DEGRADED))
    });
    Ok(OpenAppResult {
        started_window,
        sessions,
        warning: started.time_warning,
        notice,
    })
}

/// 设备与本次应使用的连接标识：已连接时用运行记录的标识（无线端口可能被重新解析过），
/// 在线时复核实际列表并按需建链，离线的无线设备先建链。
fn device_and_connection(app: &AppHandle, device_id: &str) -> Result<(Device, String), UserError> {
    let state = app.state::<AppState>();
    let device = session::find_device(&state, device_id)?;
    if let Some(record) = session::active_records(&state, device_id).into_iter().next() {
        return Ok((device, record.connection_id));
    }
    let connection_id = device.connection_id();
    if connection_id.is_empty() {
        return Err(UserError::new(codes::SESSION_CONNECTION_UNKNOWN));
    }
    match device::status_of_device(&state, device_id).as_deref() {
        Some(device::STATUS_IDLE) => {
            let present = match device::live_devices(app) {
                Ok(live) => live.contains(&connection_id),
                Err(e) => {
                    logger::warn(
                        "apps",
                        format!("读取应用列表前复核「{}」失败：{e}（沿用界面状态继续）", device.alias),
                    );
                    true
                }
            };
            if device.kind == KIND_WIFI && !present {
                let id = session::ensure_wireless_link(app, &state, &device)
                    .map_err(|e| UserError::new(codes::SESSION_LINK_FAILED).with_detail(e))?;
                return Ok((device, id));
            }
            Ok((device, connection_id))
        }
        Some(device::STATUS_OFFLINE) if device.kind == KIND_WIFI => {
            let id = session::ensure_wireless_link(app, &state, &device)
                .map_err(|e| UserError::new(codes::SESSION_LINK_FAILED).with_detail(e))?;
            Ok((device, id))
        }
        Some(device::STATUS_OFFLINE) => Err(UserError::new(codes::SESSION_DEVICE_OFFLINE)),
        Some(device::STATUS_CONNECTED) => Ok((device, connection_id)),
        Some(_) => Err(UserError::new(codes::SESSION_DEVICE_OFFLINE)),
        None => Err(UserError::new(codes::SESSION_DEVICE_NOT_FOUND)),
    }
}

fn environment_dir(app: &AppHandle) -> Result<PathBuf, UserError> {
    let dir = scrcpy::scrcpy_dir(app)
        .map_err(|e| UserError::new(codes::SESSION_ENVIRONMENT_NOT_READY).with_detail(e))?;
    let missing = assets::missing_bins(&dir);
    if !missing.is_empty() {
        return Err(
            UserError::new(codes::SESSION_ENVIRONMENT_NOT_READY).with_detail(missing.join("、"))
        );
    }
    Ok(dir)
}

struct Captured {
    text: String,
    success: bool,
}

/// 执行 `scrcpy -s <连接标识> --list-apps`，超时按进程树结束。
///
/// 输出落临时文件而不是管道：`--list-apps` 的 stdout 可能被它拉起的 adb 子进程继承，
/// 杀掉 scrcpy 后读管道的线程仍可能被那只子进程挂住（实测超时后多等了近 200 秒）。
/// 文件方案不需要常驻读线程，超时路径只等 `try_wait`。
fn capture_listing(dir: &Path, connection_id: &str) -> Result<Captured, String> {
    let bin = dir.join(scrcpy::SCRCPY_BIN);
    if !bin.is_file() {
        return Err(format!("运行环境内未找到 {}", scrcpy::SCRCPY_BIN));
    }
    let stamp = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let out_path = std::env::temp_dir().join(format!("freecontrol-apps-{stamp}.out"));
    let err_path = std::env::temp_dir().join(format!("freecontrol-apps-{stamp}.err"));
    let out_file = fs::File::create(&out_path)
        .map_err(|e| format!("创建应用列表临时文件失败：{e}"))?;
    let err_file = fs::File::create(&err_path)
        .map_err(|e| format!("创建应用列表临时文件失败：{e}"))?;

    let mut cmd = Command::new(&bin);
    cmd.arg("-s")
        .arg(connection_id)
        .arg("--list-apps")
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(out_file))
        .stderr(Stdio::from(err_file))
        .env("ADB", dir.join(ADB_BIN))
        .env("PATH", path_with(dir));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            remove_quietly(&out_path);
            remove_quietly(&err_path);
            return Err(format!("启动 scrcpy 失败：{e}"));
        }
    };

    let deadline = Instant::now() + LIST_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    kill_tree(&mut child);
                    remove_quietly(&out_path);
                    remove_quietly(&err_path);
                    return Err(format!(
                        "读取应用列表超时（超过 {} 秒）",
                        LIST_TIMEOUT.as_secs()
                    ));
                }
                std::thread::sleep(WAIT_TICK);
            }
            Err(e) => {
                kill_tree(&mut child);
                remove_quietly(&out_path);
                remove_quietly(&err_path);
                return Err(format!("等待 scrcpy 退出失败：{e}"));
            }
        }
    };

    let mut text = read_capped(&out_path);
    let err = read_capped(&err_path);
    if !err.trim().is_empty() {
        text.push('\n');
        text.push_str(&err);
    }
    remove_quietly(&out_path);
    remove_quietly(&err_path);
    Ok(Captured {
        text,
        success: status.map(|s| s.success()).unwrap_or(false),
    })
}

fn read_capped(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_default();
    let len = bytes.len().min(MAX_CAPTURE as usize);
    String::from_utf8_lossy(&bytes[..len]).into_owned()
}

fn remove_quietly(path: &Path) {
    let _ = fs::remove_file(path);
}

/// 结束 scrcpy 及其 adb 等子进程；Windows 上用 `taskkill /T` 避免残留进程占着输出文件。
fn kill_tree(child: &mut Child) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn path_with(dir: &Path) -> std::ffi::OsString {
    let base = std::env::var_os("PATH").unwrap_or_default();
    let mut parts = vec![dir.as_os_str().to_owned()];
    parts.extend(std::env::split_paths(&base).map(|p| p.into_os_string()));
    std::env::join_paths(parts).unwrap_or(base)
}

fn is_unsupported_version(output: &str) -> bool {
    let lower = output.to_lowercase();
    lower.contains("list-apps")
        && (lower.contains("unrecognized option")
            || lower.contains("unknown option")
            || lower.contains("invalid option"))
}

fn tail_of(output: &str) -> String {
    let mut tail: String = output
        .chars()
        .rev()
        .take(500)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    tail = tail.trim().to_string();
    tail.replace('\r', "")
}

/// 解析 scrcpy 的应用列表。
///
/// 行格式由服务端固定列宽生成：` * ` / ` - ` + 名称补足 30 列 + 空格 + 包名；
/// 名称达到或超过 30 列时，包名会折到下一行。按 UTF-16 列宽切分以与 Java 的 `length()`
/// 对齐（中文 BMP 字符计 1）。
pub(crate) fn parse_app_list(output: &str) -> Vec<AppEntry> {
    let mut entries = Vec::new();
    let mut lines = output.lines();
    while let Some(raw) = lines.next() {
        let line = raw.trim_end_matches('\r');
        let system = if line.starts_with(" * ") {
            true
        } else if line.starts_with(" - ") {
            false
        } else {
            continue;
        };
        let units: Vec<u16> = line.encode_utf16().collect();
        let (name, package) = if units.len() >= 35 && units[33] == b' ' as u16 {
            (
                String::from_utf16_lossy(&units[3..33]).trim().to_string(),
                String::from_utf16_lossy(&units[34..]).trim().to_string(),
            )
        } else {
            let name = String::from_utf16_lossy(&units[3..]).trim().to_string();
            let package = lines
                .next()
                .map(|l| l.trim_end_matches('\r').trim().to_string())
                .filter(|l| {
                    !l.is_empty()
                        && !l.starts_with(" * ")
                        && !l.starts_with(" - ")
                        && !l.starts_with('[')
                        && l.split_whitespace().count() == 1
                })
                .unwrap_or_default();
            (name, package)
        };
        if name.is_empty() || package.is_empty() || package.split_whitespace().count() != 1 {
            continue;
        }
        entries.push(AppEntry {
            name,
            package,
            system,
        });
    }
    entries
}

/// 一次性注入本次选择的 `--start-app`：先剔除同名旧值，再追加到最后，保证本次选择优先。
pub(crate) fn with_start_app(args: Vec<String>, package: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        if arg == "--start-app" {
            let _ = iter.next();
            continue;
        }
        if arg.starts_with("--start-app=") {
            continue;
        }
        out.push(arg);
    }
    out.push(format!("--start-app={package}"));
    out
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter()
        .any(|a| a == name || a.starts_with(&format!("{name}=")))
}

fn flag_value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    for (i, arg) in args.iter().enumerate() {
        if arg == name {
            return args.get(i + 1).map(String::as_str);
        }
        if let Some(v) = arg.strip_prefix(&format!("{name}=")) {
            return Some(v);
        }
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DisplayChoice {
    Main,
    Display(i32),
    NewVirtual,
    Headless,
}

fn display_choice(args: &[String]) -> DisplayChoice {
    if flag_value(args, "--video-source") == Some("camera") || has_flag(args, "--no-window") {
        return DisplayChoice::Headless;
    }
    if has_flag(args, "--new-display") || has_flag(args, "--flex-display") {
        return DisplayChoice::NewVirtual;
    }
    if let Some(id) = flag_value(args, "--display-id").and_then(|v| v.parse::<i32>().ok()) {
        if id > 0 {
            return DisplayChoice::Display(id);
        }
    }
    DisplayChoice::Main
}

fn choose_display(sessions: &[session::SessionRecord]) -> DisplayChoice {
    let choices: Vec<DisplayChoice> = sessions
        .iter()
        .map(|s| display_choice(&s.args))
        .collect();
    if choices.contains(&DisplayChoice::Main) {
        return DisplayChoice::Main;
    }
    if let Some(DisplayChoice::Display(id)) = choices
        .iter()
        .find(|c| matches!(c, DisplayChoice::Display(_)))
    {
        return DisplayChoice::Display(*id);
    }
    if choices.contains(&DisplayChoice::NewVirtual) {
        return DisplayChoice::NewVirtual;
    }
    DisplayChoice::Headless
}

fn valid_package(package: &str) -> bool {
    !package.is_empty()
        && package.split('.').all(|seg| {
            !seg.is_empty()
                && seg
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

fn valid_component(component: &str) -> bool {
    let Some((package, activity)) = component.split_once('/') else {
        return false;
    };
    valid_package(package)
        && !activity.is_empty()
        && activity
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '$')
}

fn parse_component(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| valid_component(line))
        .map(str::to_string)
}

/// Activity 解析与 `am start`。所有包名/组件名先经过字符校验，避免把外部输出直接拼进 shell。
fn launch_activity(
    dir: &Path,
    connection_id: &str,
    package: &str,
    display: Option<i32>,
) -> Result<(), UserError> {
    let adb = dir.join(ADB_BIN);
    if !adb.is_file() {
        return Err(UserError::new(codes::SESSION_ENVIRONMENT_NOT_READY)
            .with_detail(format!("运行环境内未找到 {ADB_BIN}")));
    }
    let resolve_args = [
        "-s",
        connection_id,
        "shell",
        "cmd",
        "package",
        "resolve-activity",
        "--brief",
        "--components",
        "-a",
        "android.intent.action.MAIN",
        "-c",
        "android.intent.category.LAUNCHER",
        package,
    ];
    let out = device::run_captured(&adb, &resolve_args, ADB_TIMEOUT)
        .map_err(|e| UserError::new(codes::APPS_OPEN_FAILED).with_detail(e))?;
    let component = parse_component(&out).ok_or_else(|| {
        UserError::new(codes::APPS_ACTIVITY_NOT_FOUND).with_detail(format!(
            "无法解析 {package} 的启动 Activity"
        ))
    })?;

    let mut args: Vec<String> = vec![
        "-s".into(),
        connection_id.into(),
        "shell".into(),
        "am".into(),
        "start".into(),
    ];
    if let Some(id) = display {
        args.push("--display".into());
        args.push(id.to_string());
    }
    args.push("-n".into());
    args.push(component);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = device::run_captured(&adb, &refs, ADB_TIMEOUT)
        .map_err(|e| UserError::new(codes::APPS_OPEN_FAILED).with_detail(e))?;
    let lower = out.to_lowercase();
    if lower.contains("error") || lower.contains("exception") {
        return Err(UserError::new(codes::APPS_OPEN_FAILED).with_detail(out.trim().to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 按服务端固定列宽生成一行：marker + 名称补足 30 列 + 空格 + 包名；
    /// 名称达到 30 列时包名折到下一行（34 个空格）。
    fn app_line(marker: &str, name: &str, package: &str) -> String {
        let width = name.encode_utf16().count();
        let pad = 30usize.saturating_sub(width);
        if pad > 0 {
            format!(" {marker} {name}{}{package}\n", " ".repeat(pad + 1))
        } else {
            format!(" {marker} {name}\n{}{package}\n", " ".repeat(34))
        }
    }

    fn sample() -> String {
        let mut s = String::from(
            "scrcpy 5.0.1 <https://github.com/Genymobile/scrcpy>\nINFO: ADB device found:\n[server] INFO: Processing Android apps... (this may take some time)\n[server] INFO: List of apps:\n",
        );
        s.push_str(&app_line("*", "设置", "com.android.settings"));
        s.push_str(&app_line("*", "相机", "com.android.camera"));
        s.push_str(&app_line("-", "F-Droid", "org.fdroid.fdroid"));
        s.push_str(&app_line("-", "企业微信", "com.tencent.wework"));
        s
    }

    #[test]
    fn parses_system_and_user_apps() {
        let apps = parse_app_list(&sample());
        assert_eq!(apps.len(), 4);
        assert_eq!(apps[0].name, "设置");
        assert_eq!(apps[0].package, "com.android.settings");
        assert!(apps[0].system);
        assert_eq!(apps[2].package, "org.fdroid.fdroid");
        assert!(!apps[2].system);
    }

    #[test]
    fn parses_a_name_that_folds_the_package_to_the_next_line() {
        // 名称达到 30 列：包名被服务端折到下一行（前面是 34 个空格）。
        let long = "超长应用名称超长应用名称超长应用名称超长应用名称超长应用名称一";
        let line = format!(" - {long}\n");
        let pad = " ".repeat(34);
        let input = format!("[server] INFO: List of apps:\n{line}{pad}com.example.long\n");
        let apps = parse_app_list(&input);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, long);
        assert_eq!(apps[0].package, "com.example.long");
    }

    #[test]
    fn ignores_logs_and_crlf() {
        let input = sample().replace('\n', "\r\n");
        let apps = parse_app_list(&input);
        assert_eq!(apps.len(), 4, "日志行不会进入清单");
    }

    #[test]
    fn injects_start_app_last_and_removes_old_values() {
        let args = vec![
            "--no-audio".to_string(),
            "--start-app=old.app".to_string(),
            "--video-codec=h265".to_string(),
            "--start-app".to_string(),
            "pair.app".to_string(),
        ];
        let out = with_start_app(args, "com.new.app");
        assert_eq!(
            out,
            vec![
                "--no-audio".to_string(),
                "--video-codec=h265".to_string(),
                "--start-app=com.new.app".to_string()
            ]
        );
    }

    #[test]
    fn classifies_display_targets() {
        assert_eq!(
            display_choice(&["--display-id=2".to_string()]),
            DisplayChoice::Display(2)
        );
        assert_eq!(
            display_choice(&["--new-display=1920x1080".to_string()]),
            DisplayChoice::NewVirtual
        );
        assert_eq!(
            display_choice(&["--video-source=camera".to_string()]),
            DisplayChoice::Headless
        );
        assert_eq!(
            display_choice(&["--no-window".to_string()]),
            DisplayChoice::Headless
        );
        assert_eq!(display_choice(&[]), DisplayChoice::Main);
    }

    #[test]
    fn isolates_new_window_when_main_display_already_mirrored() {
        let out = isolate_shared_main_display(
            vec![
                "--no-audio".to_string(),
                "--no-vd-destroy-content".to_string(),
                "--start-app=com.example".to_string(),
            ],
            &[vec!["--max-fps=30".to_string()]],
        );
        assert_eq!(
            out,
            vec![
                "--no-audio".to_string(),
                "--start-app=com.example".to_string(),
                "--new-display".to_string()
            ]
        );
    }

    #[test]
    fn keeps_new_window_args_without_shared_main_display() {
        let main = vec!["--start-app=com.example".to_string()];
        // 没有已有会话：新窗口本身就是第一条会话，照旧镜像主屏。
        assert_eq!(isolate_shared_main_display(main.clone(), &[]), main);
        // 已有会话镜像的不是主屏：互不干扰。
        assert_eq!(
            isolate_shared_main_display(main.clone(), &[vec!["--display-id=2".to_string()]]),
            main
        );
        assert_eq!(
            isolate_shared_main_display(
                main.clone(),
                &[vec!["--new-display".to_string()], vec!["--no-window".to_string()]]
            ),
            main
        );
        // 本次本就是虚拟显示器：每次都会新建一块，无需改。
        let virtual_args = vec![
            "--new-display=1920x1080".to_string(),
            "--start-app=com.example".to_string(),
        ];
        assert_eq!(
            isolate_shared_main_display(virtual_args.clone(), &[vec![]]),
            virtual_args
        );
    }

    #[test]
    fn validates_packages_and_components() {
        assert!(valid_package("com.android.settings"));
        assert!(valid_package("mark.via"));
        assert!(!valid_package("com..android"));
        assert!(!valid_package("com.android\nshell"));
        assert!(valid_component("com.android.settings/.MainSettings"));
        assert!(valid_component("org.fdroid.fdroid/.views.main.MainActivity"));
        assert!(!valid_component("com.android.settings"));
        assert!(!valid_component("com.android.settings/Act ivity"));
    }

    #[test]
    fn parses_a_component_line() {
        let out = "priority=0 preferredOrder=0 match=0x108000 specificIndex=-1 isDefault=true\n\
com.android.settings/.MainSettings\n";
        assert_eq!(
            parse_component(out).as_deref(),
            Some("com.android.settings/.MainSettings")
        );
    }

    #[test]
    fn detects_unsupported_list_apps() {
        assert!(is_unsupported_version(
            "ERROR: Unknown option '--list-apps'"
        ));
        assert!(!is_unsupported_version("[server] INFO: List of apps:\n * 设置 com.android.settings"));
    }

    fn real_env() -> Option<(PathBuf, String)> {
        let dir = std::env::var_os("FC_TEST_SCRCPY_DIR").map(PathBuf::from)?;
        let serial = std::env::var("FC_TEST_SERIAL").ok()?;
        if serial.trim().is_empty() {
            return None;
        }
        Some((dir, serial))
    }

    /// 真机端到端：推 server、列应用、解析。默认忽略；需要真实环境与设备时显式运行：
    /// `FC_TEST_SCRCPY_DIR=... FC_TEST_SERIAL=... cargo test apps::tests::real_device_listing -- --ignored --nocapture`
    #[test]
    #[ignore = "需要真实 scrcpy 运行环境与已授权设备"]
    fn real_device_listing() {
        let Some((dir, serial)) = real_env() else {
            eprintln!("skip: set FC_TEST_SCRCPY_DIR and FC_TEST_SERIAL");
            return;
        };
        let captured = capture_listing(&dir, &serial).expect("list-apps 应能完成");
        assert!(!is_unsupported_version(&captured.text));
        let apps = parse_app_list(&captured.text);
        assert!(!apps.is_empty(), "设备上应有可启动应用");
        eprintln!(
            "real listing: {} apps, first = {} ({})",
            apps.len(),
            apps[0].name,
            apps[0].package
        );
    }

    /// 真机只读校验：Activity 解析（不执行 am start，避免在设备上打开应用）。
    #[test]
    #[ignore = "需要真实 scrcpy 运行环境与已授权设备"]
    fn real_device_activity_resolution() {
        let Some((dir, serial)) = real_env() else {
            eprintln!("skip: set FC_TEST_SCRCPY_DIR and FC_TEST_SERIAL");
            return;
        };
        let adb = dir.join(ADB_BIN);
        let args = [
            "-s",
            serial.as_str(),
            "shell",
            "cmd",
            "package",
            "resolve-activity",
            "--brief",
            "--components",
            "-a",
            "android.intent.action.MAIN",
            "-c",
            "android.intent.category.LAUNCHER",
            "com.android.settings",
        ];
        let out = device::run_captured(&adb, &args, ADB_TIMEOUT).expect("resolve-activity 应能执行");
        let component = parse_component(&out).expect("应解析出启动组件");
        assert!(valid_component(&component));
        eprintln!("resolved component: {component}");
    }
}
