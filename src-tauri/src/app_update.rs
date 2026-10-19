//! 应用自更新（spec `app-update` / design D2–D7）。
//!
//! 与运行环境获取（`download`）共用通道解析、HTTP 客户端与可中断的下载原语，但**不共用**
//! 命令、状态与失败文案：scrcpy 的结果语义与「手动放置到约定目录」提示在这里都不成立。
//!
//! 流程：
//! 1. `check_app_update`：经「直连 → 配置代理」查询本项目最新正式发行，解析 tag、
//!    选择当前系统 / 架构的资产，并判断是否已有校验通过的待应用包；
//! 2. `start_app_update`：把资产下载到**最终替换位置所在目录**的临时文件，校验发行元数据里的
//!    SHA-256；通过后转为待应用包（关闭视图不会丢）；
//! 3. `apply_app_update`：Windows 单文件 / Linux AppImage 原地替换并重启；deb 交给系统安装器；
//! 4. 启动时清理残留的临时文件与备份。

use crate::download::{
    self, AttemptError, AttemptFailure, Channel, ChannelMemory, InstallCtl, resolve_channels,
    usable_channels,
};
use crate::{config, logger, scrcpy, session, shell, AppState};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tauri::{AppHandle, Emitter, Manager, State};

/// 本项目最新正式发行接口（与 scrcpy 的 `LATEST_RELEASE_API` 是两个不同的目标）。
pub const RELEASE_API: &str = "https://api.github.com/repos/pdone/FreeControl2/releases/latest";

/// 资产下载地址前缀（发行元数据缺下载地址时按此拼接）。
const RELEASES_DOWNLOAD: &str = "https://github.com/pdone/FreeControl2/releases/download";

/// 更新进度事件名 —— 与 `scrcpy-install-progress` 分开，互不干扰（design D7）。
pub const PROGRESS_EVENT: &str = "app-update-progress";

/// 临时文件 / 备份文件前缀。启动清理按此前缀识别残留。
const TMP_PREFIX: &str = ".fc2-app-update-";
const BACKUP_PREFIX: &str = ".fc2-app-update-backup-";

// ── 类型 ───────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackageKind {
    WindowsExe,
    AppImage,
    Deb,
}

/// 一次检查确定下来的下载计划：版本、资产名、原始地址与校验值。
#[derive(Clone, Debug)]
pub struct AppUpdatePlan {
    pub version: String,
    pub asset: String,
    pub url: String,
    pub digest: String,
    pub kind: PackageKind,
}

/// 校验通过、等待「立即重启并更新」的包。
pub struct PendingAppUpdate {
    pub path: PathBuf,
    pub version: String,
    pub asset: String,
    pub kind: PackageKind,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateCheck {
    pub current: String,
    pub latest: String,
    /// `upToDate` | `updateAvailable` | `installedNewer` | `ready`
    pub state: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateOutcome {
    /// `ready` | `canceled`
    pub status: String,
    pub version: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateProgress {
    /// `download` | `verify`
    pub phase: String,
    pub percent: u8,
    pub channel: String,
}

/// 命令失败：`kind` 供界面选择提示文案，`message` 只写日志、不直接展示。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateError {
    pub kind: String,
    pub message: String,
}

impl AppUpdateError {
    fn new(kind: &str, message: impl Into<String>) -> Self {
        AppUpdateError {
            kind: kind.to_string(),
            message: message.into(),
        }
    }
    fn check(message: impl Into<String>) -> Self {
        Self::new("check", message)
    }
    fn download(message: impl Into<String>) -> Self {
        Self::new("download", message)
    }
    fn no_asset(message: impl Into<String>) -> Self {
        Self::new("noAsset", message)
    }
    fn write(message: impl Into<String>) -> Self {
        Self::new("write", message)
    }
}

// ── 平台与资产选择 ─────────────────────────────────────────

pub fn current_kind() -> PackageKind {
    #[cfg(windows)]
    {
        PackageKind::WindowsExe
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if std::env::var_os("APPIMAGE").is_some() {
            PackageKind::AppImage
        } else {
            PackageKind::Deb
        }
    }
    #[cfg(not(any(windows, unix)))]
    {
        PackageKind::WindowsExe
    }
}

/// 架构段别名：Windows 用 `x64` / `arm64`，Linux 的 Tauri 产物用 `amd64` / `arm64`。
fn arch_aliases(arch: &str) -> &'static [&'static str] {
    match arch {
        "x86_64" => &["amd64", "x86_64"],
        "aarch64" => &["arm64", "aarch64"],
        _ => &[],
    }
}

/// 当前平台 / 架构 / 运行形态可能对应的资产名（按优先级排列）。
fn expected_names(version: &str, arch: &str, kind: PackageKind) -> Vec<String> {
    match kind {
        PackageKind::WindowsExe => match arch {
            "x86_64" => vec![
                format!("FreeControl2_{version}_x64.exe"),
                // v1.0.1 的历史命名：不带架构段，只可能是 x64。
                format!("FreeControl2_{version}.exe"),
            ],
            "aarch64" => vec![format!("FreeControl2_{version}_arm64.exe")],
            _ => Vec::new(),
        },
        PackageKind::AppImage => arch_aliases(arch)
            .iter()
            .map(|a| format!("FreeControl2_{version}_{a}.AppImage"))
            .collect(),
        PackageKind::Deb => arch_aliases(arch)
            .iter()
            .map(|a| format!("FreeControl2_{version}_{a}.deb"))
            .collect(),
    }
}

/// 从发行资产里按候选名挑一份；名字必须原样匹配，找不到就是没有匹配资产。
fn pick_asset<'a>(assets: &'a [ReleaseAsset], names: &[String]) -> Option<&'a ReleaseAsset> {
    names.iter().find_map(|name| {
        assets.iter().find(|asset| {
            let safe = !asset.name.contains('/') && !asset.name.contains('\\');
            safe && &asset.name == name
        })
    })
}

/// `sha256:<hex>` → 小写 hex；长度或字符不合法视为没有可用校验值。
fn parse_digest(raw: Option<&str>) -> Option<String> {
    let raw = raw?.trim();
    let hex = raw.strip_prefix("sha256:").unwrap_or(raw);
    (hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| hex.to_ascii_lowercase())
}

// ── 发行 JSON ──────────────────────────────────────────────

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    assets: Vec<ReleaseAsset>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

#[derive(Clone, Deserialize, Serialize)]
struct ReleaseAsset {
    name: String,
    #[serde(default)]
    browser_download_url: Option<String>,
    #[serde(default)]
    digest: Option<String>,
}

/// 解析发行 JSON，选择本平台资产。
///
/// `arch` / `kind` 显式传入，单测才能在任意宿主上覆盖 Windows 与 Linux 两支。
fn parse_release_for(
    body: &str,
    arch: &str,
    kind: PackageKind,
) -> Result<AppUpdatePlan, AppUpdateError> {
    let release: Release = serde_json::from_str(body)
        .map_err(|e| AppUpdateError::check(format!("发布信息解析失败：{e}")))?;
    if release.draft || release.prerelease {
        return Err(AppUpdateError::no_asset("最新发行不是正式版本"));
    }
    let tag = release.tag_name.trim();
    let version = tag.trim_start_matches('v').to_string();
    if version.is_empty() {
        return Err(AppUpdateError::check("发布信息中的版本号为空"));
    }

    let names = expected_names(&version, arch, kind);
    let asset = pick_asset(&release.assets, &names).ok_or_else(|| {
        AppUpdateError::no_asset(format!(
            "发行 v{version} 中没有当前系统与架构可用的更新包"
        ))
    })?;
    let digest = parse_digest(asset.digest.as_deref()).ok_or_else(|| {
        AppUpdateError::no_asset("更新包缺少可用的 SHA-256 校验值，暂不能自动更新")
    })?;
    let url = asset
        .browser_download_url
        .clone()
        .filter(|u| u.starts_with("https://"))
        .unwrap_or_else(|| format!("{RELEASES_DOWNLOAD}/{tag}/{}", asset.name));

    Ok(AppUpdatePlan {
        version,
        asset: asset.name.clone(),
        url,
        digest,
        kind,
    })
}

/// 版本比较：复用 scrcpy 的同一套数字段比较，展示与比较都不带前导 `v`。
fn classify_version(current: &str, latest: &str) -> Result<&'static str, AppUpdateError> {
    match scrcpy::compare_versions(current, latest) {
        Some(Ordering::Equal) => Ok("upToDate"),
        Some(Ordering::Less) => Ok("updateAvailable"),
        Some(Ordering::Greater) => Ok("installedNewer"),
        None => Err(AppUpdateError::check("无法比较当前版本与最新版本")),
    }
}

// ── 检查 ───────────────────────────────────────────────────

/// 按通道顺序取发行 JSON（自有循环，不套用 scrcpy 的尝试文案与「手动放置」提示）。
async fn fetch_release_body_with<F, Fut>(
    channels: &[Channel],
    memory: &Mutex<ChannelMemory>,
    mut fetch: F,
) -> Result<String, AppUpdateError>
where
    F: FnMut(&Channel) -> Fut,
    Fut: std::future::Future<Output = Result<String, AttemptFailure>>,
{
    let candidates = usable_channels(&memory.lock().unwrap(), channels);
    let mut failures: Vec<String> = Vec::new();
    for channel in &candidates {
        logger::info(
            "app-update",
            format!("检查更新：尝试通道 {}", channel.label()),
        );
        match fetch(channel).await {
            Ok(body) => return Ok(body),
            Err(failure) => {
                logger::warn(
                    "app-update",
                    format!(
                        "更新检查通道「{}」失败：{}{}",
                        channel.label(),
                        failure.reason,
                        if failure.unreachable {
                            "（本次运行内不再尝试该通道）"
                        } else {
                            ""
                        }
                    ),
                );
                if failure.unreachable {
                    memory.lock().unwrap().mark_unreachable(channel);
                }
                failures.push(format!("{}：{}", channel.label(), failure.reason));
            }
        }
    }
    Err(AppUpdateError::check(format!(
        "更新检查失败（{}）",
        failures.join("；")
    )))
}

fn discard_pending(state: &AppState) {
    if let Some(pending) = state.app_update_pending.lock().unwrap().take() {
        let _ = fs::remove_file(&pending.path);
    }
}

#[tauri::command]
pub async fn check_app_update(app: AppHandle) -> Result<AppUpdateCheck, AppUpdateError> {
    let current = app.package_info().version.to_string();
    let state = app.state::<AppState>();
    let proxies = state.config.lock().unwrap().channels.proxies.clone();
    let channels = resolve_channels(&proxies);
    let client = download::http_client().map_err(AppUpdateError::check)?;
    let body = fetch_release_body_with(&channels, &state.channel_memory, |channel| {
        let url = channel.url_for(RELEASE_API);
        let client = &client;
        async move { download::fetch_text(client, &url).await }
    })
    .await?;

    let plan = parse_release_for(&body, std::env::consts::ARCH, current_kind())?;
    let kind = classify_version(&current, &plan.version)?;
    logger::info(
        "app-update",
        format!(
            "检查更新完成：当前 {current}，线上 {}（{}），结果 {kind}",
            plan.version, plan.asset
        ),
    );

    let mut state_key = kind;
    if kind == "updateAvailable" {
        let ready = {
            let pending = state.app_update_pending.lock().unwrap();
            pending
                .as_ref()
                .map(|p| p.version == plan.version && p.path.exists())
                .unwrap_or(false)
        };
        if ready {
            state_key = "ready";
        } else {
            discard_pending(&state);
        }
    } else {
        // 已是最新 / 当前版本更高：待应用包都不可能再被用上。
        discard_pending(&state);
    }

    *state.app_update_plan.lock().unwrap() = Some(plan.clone());
    Ok(AppUpdateCheck {
        current,
        latest: plan.version,
        state: state_key.to_string(),
    })
}

// ── 下载与校验 ─────────────────────────────────────────────

fn emit_progress(app: &AppHandle, phase: &str, percent: u8, channel: &str) {
    let _ = app.emit(
        PROGRESS_EVENT,
        AppUpdateProgress {
            phase: phase.to_string(),
            percent,
            channel: channel.to_string(),
        },
    );
}

/// 下载落点所在目录：Windows / AppImage 必须与最终文件同目录（rename 原子），deb 放应用数据根。
fn download_dir(app: &AppHandle, kind: PackageKind) -> Result<PathBuf, AppUpdateError> {
    match kind {
        PackageKind::WindowsExe => std::env::current_exe()
            .map_err(|e| AppUpdateError::write(format!("无法定位当前程序：{e}")))?
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| AppUpdateError::write("无法定位当前程序所在目录")),
        PackageKind::AppImage => std::env::var_os("APPIMAGE")
            .map(PathBuf::from)
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .ok_or_else(|| AppUpdateError::write("无法定位 AppImage 所在目录")),
        PackageKind::Deb => {
            config::app_data_root(app).map_err(AppUpdateError::write)
        }
    }
}

/// 预检目录可写：真正的替换发生在同一目录，不能等下载完才发现写不进去。
/// 本地文件的 SHA-256 是否与发行元数据一致（抽成函数便于单测）。
fn verify_file_digest(path: &Path, expected: &str) -> Result<bool, String> {
    Ok(download::sha256_file(path)? == expected)
}

fn ensure_writable(dir: &Path) -> Result<(), AppUpdateError> {
    let probe = dir.join(format!("{TMP_PREFIX}probe-{}", std::process::id()));
    fs::write(&probe, b"")
        .map_err(|e| AppUpdateError::write(format!("当前安装位置不可写：{e}")))?;
    let _ = fs::remove_file(&probe);
    Ok(())
}

fn unix_timestamp() -> u128 {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

async fn download_update(
    app: &AppHandle,
    state: &AppState,
    ctl: Arc<InstallCtl>,
) -> Result<AppUpdateOutcome, AppUpdateError> {
    let plan = state
        .app_update_plan
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| AppUpdateError::check("请先检查更新"))?;
    let dir = download_dir(app, plan.kind)?;
    ensure_writable(&dir)?;
    // 同一时刻只有一个待应用包；上一次的残留先清掉。
    discard_pending(state);

    let tmp = dir.join(format!(
        "{TMP_PREFIX}{}-{}-{}.download",
        plan.version,
        std::process::id(),
        unix_timestamp()
    ));
    let proxies = state.config.lock().unwrap().channels.proxies.clone();
    let channels = resolve_channels(&proxies);
    let client = download::http_client().map_err(AppUpdateError::download)?;
    let candidates = usable_channels(&state.channel_memory.lock().unwrap(), &channels);

    logger::info(
        "app-update",
        format!(
            "开始下载更新 v{}（{}，通道候选 {} 条）",
            plan.version,
            plan.asset,
            candidates.len()
        ),
    );

    let mut failures: Vec<String> = Vec::new();
    let mut used_channel: Option<Channel> = None;
    for channel in &candidates {
        let url = channel.url_for(&plan.url);
        let label = channel.label();
        logger::info("app-update", format!("下载更新：尝试通道 {label}"));
        let mut last_percent: i32 = -1;
        let result = download::download_to_file(&client, &url, &tmp, &ctl, |done, total| {
            let percent = match total {
                Some(t) if t > 0 => ((done * 100) / t) as i32,
                _ => -1,
            };
            if percent == last_percent {
                return;
            }
            last_percent = percent;
            emit_progress(app, "download", percent.max(0) as u8, &label);
        })
        .await;
        match result {
            Ok(_) => {
                used_channel = Some(channel.clone());
                break;
            }
            Err(AttemptError::Canceled) => {
                let _ = fs::remove_file(&tmp);
                logger::info("app-update", "用户取消应用更新下载（临时文件已清理）");
                return Ok(AppUpdateOutcome {
                    status: "canceled".to_string(),
                    version: String::new(),
                });
            }
            // 更新视图没有「切换通道」入口；真收到也按一条通道失败处理。
            Err(AttemptError::Switched) => {
                logger::warn("app-update", format!("通道「{label}」被跳过"));
                failures.push(format!("{label}：已跳过"));
            }
            Err(AttemptError::Failure(failure)) => {
                logger::warn(
                    "app-update",
                    format!("更新下载通道「{label}」失败：{}", failure.reason),
                );
                if failure.unreachable {
                    state
                        .channel_memory
                        .lock()
                        .unwrap()
                        .mark_unreachable(channel);
                }
                failures.push(format!("{label}：{}", failure.reason));
            }
        }
    }

    let Some(channel) = used_channel else {
        let _ = fs::remove_file(&tmp);
        return Err(AppUpdateError::download(format!(
            "更新包下载失败（{}）",
            failures.join("；")
        )));
    };

    emit_progress(app, "verify", 100, &channel.label());
    let matches = verify_file_digest(&tmp, &plan.digest).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        AppUpdateError::download(format!("校验更新包失败：{e}"))
    })?;
    if !matches {
        let _ = fs::remove_file(&tmp);
        logger::error(
            "app-update",
            format!("{} 校验不通过，已删除下载文件", plan.asset),
        );
        return Err(AppUpdateError::download("更新包校验未通过，请重试"));
    }

    #[cfg(unix)]
    if plan.kind == PackageKind::AppImage {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755)).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            AppUpdateError::write(format!("无法赋予更新包执行权限：{e}"))
        })?;
    }

    logger::info(
        "app-update",
        format!(
            "更新包已校验并保留待应用：v{}（{}，通道 {}）",
            plan.version,
            plan.asset,
            channel.label()
        ),
    );
    *state.app_update_pending.lock().unwrap() = Some(PendingAppUpdate {
        path: tmp,
        version: plan.version.clone(),
        asset: plan.asset.clone(),
        kind: plan.kind,
    });
    Ok(AppUpdateOutcome {
        status: "ready".to_string(),
        version: plan.version,
    })
}

#[tauri::command]
pub async fn start_app_update(app: AppHandle) -> Result<AppUpdateOutcome, AppUpdateError> {
    let state = app.state::<AppState>();
    {
        let mut slot = state.app_update_ctl.lock().unwrap();
        if slot.is_some() {
            return Err(AppUpdateError::download("已有更新正在下载，请稍候"));
        }
        *slot = Some(Arc::new(InstallCtl::new()));
    }
    let ctl = state.app_update_ctl.lock().unwrap().clone().unwrap();
    let result = download_update(&app, &state, ctl).await;
    *state.app_update_ctl.lock().unwrap() = None;
    result
}

/// 取消在途下载。已校验完成的待应用包不受影响（design D5：关闭视图后仍可重启更新）。
#[tauri::command]
pub fn cancel_app_update(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(ctl) = state.app_update_ctl.lock().unwrap().clone() {
        logger::info("app-update", "用户请求取消应用更新下载");
        ctl.request_cancel();
    }
    Ok(())
}

// ── 应用与重启 ─────────────────────────────────────────────

fn replace_target(kind: PackageKind) -> Result<PathBuf, AppUpdateError> {
    match kind {
        PackageKind::WindowsExe => std::env::current_exe()
            .map_err(|e| AppUpdateError::write(format!("无法定位当前程序：{e}"))),
        PackageKind::AppImage => std::env::var_os("APPIMAGE")
            .map(PathBuf::from)
            .ok_or_else(|| AppUpdateError::write("无法定位当前 AppImage")),
        PackageKind::Deb => Err(AppUpdateError::write("deb 不执行文件替换")),
    }
}

fn replace_and_restart(app: &AppHandle, pending: &PendingAppUpdate) -> Result<(), AppUpdateError> {
    let target = replace_target(pending.kind)?;
    let dir = target
        .parent()
        .ok_or_else(|| AppUpdateError::write("无法定位程序所在目录"))?;
    let backup = dir.join(format!(
        "{BACKUP_PREFIX}{}-{}-{}",
        pending.version,
        std::process::id(),
        unix_timestamp()
    ));

    fs::rename(&target, &backup)
        .map_err(|e| AppUpdateError::write(format!("无法替换正在运行的程序：{e}")))?;
    if let Err(e) = fs::rename(&pending.path, &target) {
        let _ = fs::rename(&backup, &target);
        return Err(AppUpdateError::write(format!("无法写入新版本程序：{e}")));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = fs::set_permissions(&target, fs::Permissions::from_mode(0o755)) {
            let _ = fs::remove_file(&target);
            let _ = fs::rename(&backup, &target);
            return Err(AppUpdateError::write(format!("无法赋予新版本执行权限：{e}")));
        }
    }

    // 会话随应用退出结束；重启前先落盘配置，避免去抖窗口里的改动丢失。
    let cfg = app.state::<AppState>().config.lock().unwrap().clone();
    if let Err(e) = config::save_config_to_disk(app, &cfg) {
        logger::warn("app-update", format!("重启更新前保存配置失败：{e}"));
    }
    session::shutdown_all(app);

    match std::process::Command::new(&target).spawn() {
        Ok(_) => {
            logger::info(
                "app-update",
                format!(
                    "已替换为 v{} 并启动新进程（{}）",
                    pending.version,
                    target.display()
                ),
            );
            app.exit(0);
            Ok(())
        }
        Err(e) => {
            // 新文件已落到原路径但拉不起来：把旧文件换回去，避免「更新了却打不开」。
            let _ = fs::remove_file(&target);
            let _ = fs::rename(&backup, &target);
            Err(AppUpdateError::write(format!("新版本启动失败：{e}")))
        }
    }
}

#[tauri::command]
pub fn apply_app_update(app: AppHandle) -> Result<(), AppUpdateError> {
    let pending = app
        .state::<AppState>()
        .app_update_pending
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| AppUpdateError::download("没有已下载的更新包，请重新检查更新"))?;

    match pending.kind {
        PackageKind::Deb => {
            if let Err(e) = shell::open_file(&pending.path) {
                // 打开失败就把待应用包放回去，用户可以再点一次。
                *app.state::<AppState>().app_update_pending.lock().unwrap() = Some(pending);
                return Err(AppUpdateError::write(format!("无法交给系统安装器：{e}")));
            }
            logger::info(
                "app-update",
                format!("已把 {} 交给系统安装器", pending.asset),
            );
            Ok(())
        }
        _ => replace_and_restart(&app, &pending),
    }
}

// ── 启动清理 ───────────────────────────────────────────────

/// 清理上次运行残留：未应用的下载包、替换时的旧版本备份。
///
/// 待应用包只存在于内存槽位里；应用正常退出后再启动，它已经没有用武之地，必须删掉。
pub fn cleanup_stale(app: &AppHandle) {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(root) = config::app_data_root(app) {
        dirs.push(root);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.to_path_buf());
        }
    }
    if let Some(appimage) = std::env::var_os("APPIMAGE").map(PathBuf::from) {
        if let Some(dir) = appimage.parent() {
            dirs.push(dir.to_path_buf());
        }
    }
    dirs.sort();
    dirs.dedup();
    for dir in dirs {
        cleanup_stale_in(&dir);
    }
}

/// 清理单个目录下本模块产生的残留（纯文件操作，便于单测）。
fn cleanup_stale_in(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(TMP_PREFIX) || name.starts_with(BACKUP_PREFIX) {
            let path = entry.path();
            if fs::remove_file(&path).is_ok() {
                logger::info("app-update", format!("已清理残留文件 {}", path.display()));
            }
        }
    }
}

// ── 单测 ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;

    fn block_on<F: Future>(fut: F) -> F::Output {
        tauri::async_runtime::block_on(fut)
    }

    fn asset(name: &str, digest: Option<&str>) -> ReleaseAsset {
        ReleaseAsset {
            name: name.to_string(),
            browser_download_url: Some(format!("https://example.test/{name}")),
            digest: digest.map(|d| d.to_string()),
        }
    }

    fn body(tag: &str, assets: Vec<ReleaseAsset>) -> String {
        serde_json::json!({ "tag_name": tag, "assets": assets }).to_string()
    }

    fn digest(seed: char) -> String {
        format!("sha256:{}", seed.to_string().repeat(64))
    }

    #[test]
    fn windows_asset_prefers_arch_and_accepts_legacy_x64_name() {
        let assets = vec![
            asset("FreeControl2_1.0.2_arm64.exe", Some(&digest('a'))),
            asset("FreeControl2_1.0.2_x64.exe", Some(&digest('b'))),
        ];
        let plan =
            parse_release_for(&body("v1.0.2", assets), "x86_64", PackageKind::WindowsExe).unwrap();
        assert_eq!(plan.asset, "FreeControl2_1.0.2_x64.exe");
        assert_eq!(plan.version, "1.0.2");
        assert_eq!(plan.digest, "b".repeat(64));

        // v1.0.1 只有不带架构段的旧命名，x64 仍能识别。
        let legacy = vec![asset("FreeControl2_1.0.1.exe", Some(&digest('c')))];
        let plan =
            parse_release_for(&body("v1.0.1", legacy), "x86_64", PackageKind::WindowsExe).unwrap();
        assert_eq!(plan.asset, "FreeControl2_1.0.1.exe");

        // ARM64 不会被旧命名糊弄。
        let legacy = vec![asset("FreeControl2_1.0.1.exe", Some(&digest('c')))];
        assert!(parse_release_for(&body("v1.0.1", legacy), "aarch64", PackageKind::WindowsExe).is_err());
    }

    #[test]
    fn windows_arm64_never_picks_x64() {
        let assets = vec![
            asset("FreeControl2_1.0.2_x64.exe", Some(&digest('a'))),
            asset("FreeControl2_1.0.2_arm64.exe", Some(&digest('b'))),
        ];
        let plan =
            parse_release_for(&body("v1.0.2", assets), "aarch64", PackageKind::WindowsExe).unwrap();
        assert_eq!(plan.asset, "FreeControl2_1.0.2_arm64.exe");
    }

    #[test]
    fn linux_picks_appimage_or_deb_by_arch() {
        let assets = vec![
            asset("FreeControl2_1.0.2_amd64.AppImage", Some(&digest('a'))),
            asset("FreeControl2_1.0.2_amd64.deb", Some(&digest('b'))),
            asset("FreeControl2_1.0.2_arm64.AppImage", Some(&digest('c'))),
        ];
        let appimage = parse_release_for(
            &body("v1.0.2", assets.clone()),
            "x86_64",
            PackageKind::AppImage,
        )
        .unwrap();
        assert_eq!(appimage.asset, "FreeControl2_1.0.2_amd64.AppImage");
        let deb =
            parse_release_for(&body("v1.0.2", assets), "x86_64", PackageKind::Deb).unwrap();
        assert_eq!(deb.asset, "FreeControl2_1.0.2_amd64.deb");
    }

    #[test]
    fn no_matching_asset_is_reported_as_no_asset() {
        let assets = vec![asset("FreeControl2_1.0.2_x64.exe", Some(&digest('a')))];
        let err =
            parse_release_for(&body("v1.0.2", assets), "aarch64", PackageKind::WindowsExe)
                .unwrap_err();
        assert_eq!(err.kind, "noAsset");
        assert!(err.message.contains("没有当前系统与架构可用的更新包"));
    }

    #[test]
    fn missing_or_malformed_digest_is_refused() {
        for raw in [None, Some("sha256:abc"), Some("md5:deadbeef")] {
            let assets = vec![asset("FreeControl2_1.0.2_x64.exe", raw)];
            let err = parse_release_for(&body("v1.0.2", assets), "x86_64", PackageKind::WindowsExe)
                .unwrap_err();
            assert_eq!(err.kind, "noAsset");
            assert!(err.message.contains("SHA-256"));
        }
    }

    #[test]
    fn url_falls_back_to_release_download_prefix() {
        let assets = vec![ReleaseAsset {
            name: "FreeControl2_1.0.2_x64.exe".to_string(),
            browser_download_url: None,
            digest: Some(digest('a')),
        }];
        let plan =
            parse_release_for(&body("v1.0.2", assets), "x86_64", PackageKind::WindowsExe).unwrap();
        assert_eq!(
            plan.url,
            "https://github.com/pdone/FreeControl2/releases/download/v1.0.2/FreeControl2_1.0.2_x64.exe"
        );
    }

    #[test]
    fn version_classification_matches_release_semantics() {
        assert_eq!(classify_version("1.0.1", "1.0.1").unwrap(), "upToDate");
        assert_eq!(
            classify_version("1.0.1", "1.0.2").unwrap(),
            "updateAvailable"
        );
        assert_eq!(
            classify_version("1.0.3", "1.0.2").unwrap(),
            "installedNewer"
        );
        assert!(classify_version("dev", "1.0.2").is_err());
    }

    #[test]
    fn check_falls_back_to_proxy_and_remembers_unreachable_channel() {
        let channels = vec![
            Channel::Official,
            Channel::Proxy {
                url: "https://mirror.test".to_string(),
            },
        ];
        let memory = Mutex::new(ChannelMemory::default());
        let body = block_on(fetch_release_body_with(&channels, &memory, |channel| {
            let official = *channel == Channel::Official;
            async move {
                if official {
                    Err(AttemptFailure {
                        reason: "无法建立连接".to_string(),
                        unreachable: true,
                    })
                } else {
                    Ok("release-body".to_string())
                }
            }
        }))
        .unwrap();
        assert_eq!(body, "release-body");
        assert!(memory.lock().unwrap().skipped(&Channel::Official));
    }

    #[test]
    fn check_reports_all_channels_failed() {
        let channels = vec![Channel::Official];
        let memory = Mutex::new(ChannelMemory::default());
        let err = block_on(fetch_release_body_with(&channels, &memory, |_| async {
            Err(AttemptFailure {
                reason: "请求超时".to_string(),
                unreachable: false,
            })
        }))
        .unwrap_err();
        assert_eq!(err.kind, "check");
        assert!(err.message.contains("GitHub 直连"));
    }

    #[test]
    fn stale_files_use_the_cleanup_prefixes() {
        assert!(format!("{TMP_PREFIX}1.0.2-1.download").starts_with(TMP_PREFIX));
        assert!(format!("{BACKUP_PREFIX}1.0.2-1").starts_with(BACKUP_PREFIX));
    }

    #[test]
    fn digest_verification_accepts_the_expected_hash_only() {
        let path = std::env::temp_dir().join(format!(
            "fc2-app-update-digest-{}-{}",
            std::process::id(),
            unix_timestamp()
        ));
        fs::write(&path, b"abc").unwrap();
        // SHA-256("abc") 的标准取值。
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(verify_file_digest(&path, expected).unwrap());
        assert!(!verify_file_digest(&path, &"0".repeat(64)).unwrap());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn cleanup_removes_only_this_modules_stale_files() {
        let dir = std::env::temp_dir().join(format!(
            "fc2-app-update-clean-{}-{}",
            std::process::id(),
            unix_timestamp()
        ));
        fs::create_dir_all(&dir).unwrap();
        let tmp = dir.join(format!("{TMP_PREFIX}1.0.2.download"));
        let backup = dir.join(format!("{BACKUP_PREFIX}1.0.1.exe"));
        let keep = dir.join("FreeControl2_1.0.1.exe");
        fs::write(&tmp, b"x").unwrap();
        fs::write(&backup, b"x").unwrap();
        fs::write(&keep, b"x").unwrap();

        cleanup_stale_in(&dir);
        assert!(!tmp.exists(), "临时下载文件应被清理");
        assert!(!backup.exists(), "替换备份应被清理");
        assert!(keep.exists(), "非本模块文件不得删除");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn digest_parser_normalizes_case_and_prefix() {
        let hex = "ABCDEF".repeat(10) + "ABCD";
        assert_eq!(
            parse_digest(Some(&format!("sha256:{hex}"))),
            Some(hex.to_ascii_lowercase())
        );
        assert_eq!(parse_digest(Some(&hex)), Some(hex.to_ascii_lowercase()));
        assert_eq!(parse_digest(Some("sha256:xyz")), None);
        assert_eq!(parse_digest(None), None);
    }
}
