//! 运行环境与资产辅助（tasks 3.1–3.4 / design D4）。
//!
//! 职责：
//! - 把运行环境目录（`%APPDATA%/FreeControl2/scrcpy-{OS}-v{Version}`）交给前端，
//!   并给出「是否就绪」的判定结果 —— 判定只在这里做一次，前端只渲染；
//! - 「打开目录」交互，方便用户自行查看 / 手动放置二进制；
//! - 首启缺失引导所需的全部信息（目录 + 就绪 + 缺失清单）。
//!
//! 命名与 `ui-design/launcher-prototype.html` 中的注释保持一致：
//! `assets::get_scrcpy_dir()` / `assets::open_scrcpy_dir()`。

use crate::scrcpy;
use crate::shell;
use serde::Serialize;
use std::fs;
use std::path::Path;
use tauri::AppHandle;

/// adb 可执行文件名。adb **不随 exe 分发**，而是由 scrcpy 发布包一并提供（proposal Impact）。
pub const ADB_BIN: &str = if cfg!(windows) { "adb.exe" } else { "adb" };

/// 运行环境就绪所必需的可执行文件，缺一即视为未就绪（tasks 3.2 明确要求含 `adb`）。
pub const REQUIRED_BINS: [&str; 2] = [scrcpy::SCRCPY_BIN, ADB_BIN];

/// 运行环境状态对象（前端只渲染，不做判定）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvStatus {
    /// 约定目录的完整路径（无论是否就绪都返回，便于用户手动放置）
    pub dir: String,
    /// 界面「约定目录」的说明文案（按平台拼好，前端逐字渲染，MUST NOT 自拼平台前缀）
    pub dir_hint: String,
    /// 目录内必需可执行文件是否齐备
    pub ready: bool,
    /// 缺失的必需可执行文件名（就绪时为空）
    pub missing: Vec<String>,
}

/// 目录内缺失的必需可执行文件（就绪时为空列表）。
///
/// 只做「是否存在」判定，不执行文件 —— 可执行性与版本由 `scrcpy::detect_installed_version`
/// 的探测链负责，二者互不干扰：文件在但跑不起来时，环境依旧算就绪（版本退化为 `unknown` 并给警示）。
pub fn missing_bins(dir: &Path) -> Vec<String> {
    REQUIRED_BINS
        .iter()
        .filter(|name| !dir.join(name).is_file())
        .map(|name| (*name).to_string())
        .collect()
}

/// 应用数据根的**展示前缀**（design D8）。
///
/// 只用于界面文案，不参与任何路径解析 —— 路径一律由 `config::app_data_root` 解析；
/// 两者必须一致但来源不同（一个是给人看的写法，一个是真实目录）。
fn data_root_display() -> &'static str {
    if cfg!(windows) {
        "%APPDATA%"
    } else {
        // Tauri 的 `data_dir()` 在 Linux 上取 `$XDG_DATA_HOME`，未设时为 `~/.local/share`。
        "$XDG_DATA_HOME"
    }
}

/// 界面上「约定目录」的说明文案（design D8）：由后端按平台拼好下发，前端逐字渲染。
///
/// Windows 上逐字为 `%APPDATA%/FreeControl2/scrcpy-{OS}-v{Version}` —— 与本次变更之前
/// 完全一致，含 `{OS}` / `{Version}` 占位符（这是「约定」而非「当前值」，故保留占位符写法，
/// 不顺手替换成具体标签）。Linux 上以 XDG 数据根呈现，因此同一句文案不会出现别的平台的
/// 数据根前缀。
pub fn dir_hint(data_root_display: &str) -> String {
    format!("{data_root_display}/FreeControl2/scrcpy-{{OS}}-v{{Version}}")
}

/// 由目录推导状态对象（纯函数，便于单测 tasks 3.2 的三种情况）。
///
/// 就绪判定只做一次扫描，`ready` 与 `missing` 必然一致（同源于 `missing_bins`）。
/// 目录不存在时全部必需文件都算缺失，故无需单独判断目录 —— 空目录也不会被当作就绪。
pub fn build_env_status(dir: &Path, dir_hint: &str) -> EnvStatus {
    let missing = missing_bins(dir);
    EnvStatus {
        dir: dir.to_string_lossy().to_string(),
        dir_hint: dir_hint.to_string(),
        ready: missing.is_empty(),
        missing,
    }
}

#[tauri::command]
pub fn get_scrcpy_dir(app: AppHandle) -> Result<String, String> {
    Ok(scrcpy::scrcpy_dir(&app)?.to_string_lossy().to_string())
}

/// 运行环境状态命令（tasks 3.2）：首启判定与「重新检测」共用同一条路径。
#[tauri::command(async)]
pub fn get_env_status(app: AppHandle) -> Result<EnvStatus, String> {
    let dir = scrcpy::scrcpy_dir(&app)?;
    Ok(build_env_status(&dir, &dir_hint(data_root_display())))
}

/// 在系统文件管理器中定位运行环境目录（tasks 3.3）。
///
/// 目录不存在时先创建：该操作也是「首启手动放置二进制」的入口，先建目录可省掉用户手工建目录，
/// 且空目录不会被判为就绪（见 `build_env_status`）。
///
/// 打开动作见 `shell`：Windows 上是一次 `ShellExecuteW`（`explore` 动词，打开该目录本身）。
#[tauri::command]
pub fn open_scrcpy_dir(app: AppHandle) -> Result<(), String> {
    let dir = scrcpy::scrcpy_dir(&app)?;
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    shell::open_dir(&dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "fcn-env-{}-{}-{}",
            tag,
            std::process::id(),
            nanos
        ))
    }

    #[test]
    fn absent_dir_is_not_ready() {
        // 情况一：目录不存在 —— 两个必需可执行文件都算缺失。
        let dir = unique_temp_dir("absent");
        let status = build_env_status(&dir, "");

        assert!(!status.ready);
        assert_eq!(status.missing.len(), REQUIRED_BINS.len());
        assert!(status.missing.contains(&scrcpy::SCRCPY_BIN.to_string()));
        assert!(status.missing.contains(&ADB_BIN.to_string()));
        assert_eq!(status.dir, dir.to_string_lossy());
    }

    #[test]
    fn partial_dir_reports_only_missing_ones() {
        // 情况二：目录里有 scrcpy 但缺 adb —— 未就绪，且只报缺 adb。
        let dir = unique_temp_dir("partial");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(scrcpy::SCRCPY_BIN), b"stub").unwrap();

        let status = build_env_status(&dir, "");
        let _ = fs::remove_dir_all(&dir);

        assert!(!status.ready);
        assert_eq!(status.missing, vec![ADB_BIN.to_string()]);
    }

    #[test]
    fn complete_dir_is_ready() {
        // 情况三：两个必需可执行文件齐备 —— 就绪。
        let dir = unique_temp_dir("complete");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(scrcpy::SCRCPY_BIN), b"stub").unwrap();
        fs::write(dir.join(ADB_BIN), b"stub").unwrap();

        let status = build_env_status(&dir, "");
        let _ = fs::remove_dir_all(&dir);

        assert!(status.ready);
        assert!(status.missing.is_empty());
    }

    #[test]
    fn empty_dir_is_not_ready() {
        // 空目录（仅由「打开目录」创建的空壳）不得被当作就绪。
        let dir = unique_temp_dir("empty");
        fs::create_dir_all(&dir).unwrap();

        let ready = build_env_status(&dir, "").ready;
        let _ = fs::remove_dir_all(&dir);

        assert!(!ready);
    }

    #[test]
    fn dir_hint_is_verbatim_on_windows() {
        // tasks 3.1：Windows 的说明文案逐字与本变更之前一致（含 `{OS}` / `{Version}` 占位符）。
        assert_eq!(
            dir_hint("%APPDATA%"),
            "%APPDATA%/FreeControl2/scrcpy-{OS}-v{Version}"
        );
    }

    #[test]
    fn dir_hint_uses_the_platforms_data_root_prefix() {
        // Linux 用 XDG 数据根呈现；同一句文案里 MUST NOT 出现别的平台的前缀。
        let hint = dir_hint("$XDG_DATA_HOME");
        assert_eq!(hint, "$XDG_DATA_HOME/FreeControl2/scrcpy-{OS}-v{Version}");
        assert!(!hint.contains("%APPDATA%"));
        // 展示前缀与真实数据根是两条来源：一致性由实机验收核对（tasks 9.1 / 9.2）。
        let expected = if cfg!(windows) { "%APPDATA%" } else { "$XDG_DATA_HOME" };
        assert_eq!(data_root_display(), expected);
    }
}
