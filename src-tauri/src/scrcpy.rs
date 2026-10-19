//! scrcpy 运行环境与版本管理。
//!
//! ## 版本号单一来源（design D2 / tasks 2.1）
//! 本模块的 `BEST_FIT_VERSION` 是全仓库**唯一**出现 scrcpy 版本字面量的地方，
//! 取值以只读参考源码 `doc/scrcpy/meson.build` 的 `version: '5.0.1'` 为准。
//! 前端一律通过 `get_compat_status` 等命令读取，Svelte / HTML 中不得出现 `4.1` 等版本串；
//! 升级最佳适配版本时只改 `BEST_FIT_VERSION` 一处即可。
//!
//! ## 运行环境目录（tasks 3.1）
//! 目录为「应用数据根 + `scrcpy-{平台标签}-v{Version}`」，数据根统一由 `config::app_data_root`
//! 解析；本模块只负责把根拼成运行环境目录（`scrcpy_dir_at`）。
//! **平台标签由「操作系统 × 架构」两个维度查表得到**（`PLATFORMS`，本次落 Windows 与 Linux
//! 各一行）：同一个标签同时决定运行环境目录名与官方发布资产名，二者 MUST NOT 各自取值。
//! 对外的 `get_scrcpy_dir` / `get_env_status` / `open_scrcpy_dir` 三个命令落在 `assets` 模块
//! （见 Group 3，命名与原型注释 `assets::get_scrcpy_dir()` 一致）。
//!
//! ## 已安装版本探测（design D2 实现要点 / tasks 2.2–2.3）
//! 探测链按序回退，任一环成功即返回，全部失败标记为 `unknown`：
//! 1. 执行运行环境目录内的 `scrcpy --version` 并解析输出（权威来源）；
//! 2. 回退解析运行环境目录名后缀（`scrcpy-{OS}-v{Version}`，兼容不带 `v` 的写法），
//!    仅当该目录**存在且非空**时生效 —— 否则目录名只是我们自己的构造，不构成「已安装」证据；
//! 3. 标记 `unknown`（界面给出黄色叹号，见 design D1，不阻断任何操作）。

use serde::Serialize;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Manager};

/// 一个受支持的平台：**操作系统 × 架构**两个维度共同决定官方平台标签与资产压缩格式。
///
/// 为什么是查表而不是一个常量或 `cfg!` 三元式：官方标签本身不成规律 —— Windows 的 `win64`
/// 不带架构段，Linux 的 `linux-x86_64` 带，两者无法用同一段推导表达（design D1）。
/// 表项同时就是「受支持范围」：查不到的组合由 [`current`] / [`label_at`] 明确报错，MUST NOT 兜底猜一个标签
/// （猜错的失效方式是在某个平台上安静地取回另一个平台的包）。
#[derive(Debug)]
pub struct Platform {
    /// 与 `std::env::consts::OS` 同名的操作系统取值
    pub os: &'static str,
    /// 与 `std::env::consts::ARCH` 同名的架构取值
    pub arch: &'static str,
    /// 官方发布资产与运行环境目录名里的平台标签
    /// （五种取值的权威清单见 `doc/scrcpy/release/generate_checksums.sh`）
    pub tag: &'static str,
    /// 官方发布资产的压缩后缀（不带点），解压入口据此分派
    pub ext: &'static str,
}

/// 受支持的平台表。落 Windows（x86_64 与 ARM64）与 Linux x86_64 三行；
/// macOS 的行留到实现那一平台时再补。
///
/// Windows ARM64 的官方标签是 `winarm64`（scrcpy 5.0 起提供该资产，
/// 见 `doc/scrcpy/release/generate_checksums.sh`），压缩格式同为 `.zip`。
const PLATFORMS: &[Platform] = &[
    Platform { os: "windows", arch: "x86_64", tag: "win64", ext: "zip" },
    Platform { os: "windows", arch: "aarch64", tag: "winarm64", ext: "zip" },
    Platform { os: "linux", arch: "x86_64", tag: "linux-x86_64", ext: "tar.gz" },
];

/// 按「操作系统 × 架构」查平台（纯函数，便于在任一平台上覆盖两行取值）。
pub fn label_at(os: &str, arch: &str) -> Result<&'static Platform, String> {
    PLATFORMS
        .iter()
        .find(|p| p.os == os && p.arch == arch)
        .ok_or_else(|| format!("当前平台暂不受支持：{os}/{arch}"))
}

/// 当前运行平台的表项。
pub fn current() -> Result<&'static Platform, String> {
    label_at(std::env::consts::OS, std::env::consts::ARCH)
}

/// 运行环境目录名前缀。目录名（`scrcpy-{平台标签}-v{Version}`）与「替换时留下的旧目录备份」
/// 命名都据此判定，故此处是唯一来源。
pub const DIR_PREFIX: &str = "scrcpy-";

/// 最佳适配版本（best-fit）—— 当前取值与 `doc/scrcpy/meson.build` 的 `version: '5.0.1'` 一致。
/// 全仓库唯一的版本字面量来源。
pub const BEST_FIT_VERSION: &str = "5.0.1";

/// scrcpy 可执行文件名（按平台分流）。
pub const SCRCPY_BIN: &str = if cfg!(windows) { "scrcpy.exe" } else { "scrcpy" };

/// 探测失败时的占位标记（tasks 2.3）。
pub const UNKNOWN_VERSION: &str = "unknown";

/// **生效目录**：用户自定义目录优先，否则为约定目录（tasks 3.1）。
///
/// 自定义目录可能指向一份已有的 scrcpy 运行环境（便携版、其它磁盘、多个工具共用），
/// 因此不要求其目录名符合约定命名；就绪判定与版本探测只看目录内容。
/// 数据根由 `config::app_data_root` 统一解析，保证与 `config.json` 同根。
pub fn scrcpy_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let root = crate::config::app_data_root(app)?;
    let custom = app
        .state::<crate::AppState>()
        .config
        .lock()
        .unwrap()
        .scrcpy_dir
        .clone();
    resolve_scrcpy_dir(&root, custom.as_deref())
}

/// 解析生效目录（纯函数，便于单测）。
///
/// 只有**绝对路径**且「不存在或确实是目录」的自定义值才会被采纳；相对路径或指向既有文件的
/// 值视为配置被手工改坏，记 WARN 后回落到约定目录 —— 一条坏字段不该让应用无法启动。
pub fn resolve_scrcpy_dir(root: &Path, custom: Option<&str>) -> Result<PathBuf, String> {
    if let Some(path) = valid_custom_dir(custom) {
        return Ok(path.to_path_buf());
    }
    if custom.map(str::trim).is_some_and(|s| !s.is_empty()) {
        crate::logger::warn(
            "env",
            format!(
                "配置中的自定义 scrcpy 目录不可用（须为绝对路径且不是文件），已回落到约定目录：{}",
                custom.unwrap_or_default().trim()
            ),
        );
    }
    scrcpy_dir_at(root)
}

/// 当前配置中被采纳的自定义目录；`None` 表示使用约定目录。
///
/// 与 [`resolve_scrcpy_dir`] 共用同一条「什么算有效自定义值」的判定，避免界面上的
/// 「是否为自定义」与实际生效目录各算一套。
pub fn custom_dir(app: &AppHandle) -> Option<PathBuf> {
    let raw = app
        .state::<crate::AppState>()
        .config
        .lock()
        .unwrap()
        .scrcpy_dir
        .clone();
    valid_custom_dir(raw.as_deref()).map(Path::to_path_buf)
}

fn valid_custom_dir(custom: Option<&str>) -> Option<&Path> {
    let raw = custom?.trim();
    if raw.is_empty() {
        return None;
    }
    let path = Path::new(raw);
    if !path.is_absolute() || (path.exists() && !path.is_dir()) {
        return None;
    }
    Some(path)
}

/// 由应用数据根目录派生运行环境目录（纯函数，便于单测 tasks 3.1）。
///
/// 平台不受支持时返回 `Err`：目录名由平台标签决定，取不出标签就不该落一个目录。
pub fn scrcpy_dir_at(root: &Path) -> Result<PathBuf, String> {
    Ok(root.join(dir_name_at(current()?, BEST_FIT_VERSION)))
}

/// 运行环境目录名（纯函数，接平台参数便于在任一平台上验证两行取值）。
pub fn dir_name_at(platform: &Platform, version: &str) -> String {
    format!("{}{}-v{}", DIR_PREFIX, platform.tag, version)
}

// ── 版本解析（纯函数，便于单测） ───────────────────────────

/// 版本串是否形如 `4.1` / `4.1.2`（纯数字点分段，无空段）。
fn is_version_like(s: &str) -> bool {
    !s.is_empty()
        && s.starts_with(|c: char| c.is_ascii_digit())
        && s.ends_with(|c: char| c.is_ascii_digit())
        && s.chars().all(|c| c.is_ascii_digit() || c == '.')
        && !s.contains("..")
}

/// 从 `scrcpy --version` 的输出中解析版本号。
///
/// 输出首行形如 `scrcpy 4.1 <https://github.com/Genymobile/scrcpy>`
/// （来源：`doc/scrcpy/app/src/main.c`；`--version` 另附依赖清单，见 `version.c`）。
fn parse_version_output(output: &str) -> Option<String> {
    for line in output.lines() {
        let mut tokens = line.split_whitespace();
        while let Some(tok) = tokens.next() {
            if tok.eq_ignore_ascii_case("scrcpy") {
                if let Some(next) = tokens.next() {
                    let candidate = next.trim_start_matches('v');
                    if is_version_like(candidate) {
                        return Some(candidate.to_string());
                    }
                }
            }
        }
    }
    None
}

/// 从运行环境目录名后缀解析版本号（tasks 2.3）。
///
/// 目录名形如 `scrcpy-win64-v4.1`（与官方发布包同名），也兼容不带 `v` 的 `scrcpy-win64-4.1`；
/// 版本段之后的附加后缀（如 `-portable`）被忽略。
///
/// 只认**当前平台**的标签：别的平台的目录即使存在于数据根下，也不属于本平台（design D3）。
fn parse_version_from_dir_name(dir: &Path) -> Option<String> {
    parse_version_from_dir_name_at(dir, current().ok()?.tag)
}

/// 按给定平台标签解析目录名里的版本（纯函数，便于在任一平台上覆盖两行取值）。
fn parse_version_from_dir_name_at(dir: &Path, tag: &str) -> Option<String> {
    let name = dir.file_name()?.to_str()?;
    let rest = name.strip_prefix(&format!("{}{}-", DIR_PREFIX, tag))?;
    let rest = rest.strip_prefix('v').unwrap_or(rest);
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    let candidate = rest[..end].trim_end_matches('.');
    if is_version_like(candidate) {
        Some(candidate.to_string())
    } else {
        None
    }
}

/// 版本串 → 数值分段；含非数字段时返回 `None`。
fn version_segments(v: &str) -> Option<Vec<u64>> {
    if !is_version_like(v) {
        return None;
    }
    v.split('.').map(|s| s.parse::<u64>().ok()).collect()
}

/// 比较两个版本号；任一不可解析时返回 `None`。
/// 分段不足的一方按 0 补齐，故 `4.1` 与 `4.1.0` 视为一致。
///
/// 供本模块与「检查更新」（`download::classify_update`）共用，保证版本比较规则只有一份。
pub(crate) fn compare_versions(a: &str, b: &str) -> Option<Ordering> {
    let mut sa = version_segments(a)?;
    let mut sb = version_segments(b)?;
    let len = sa.len().max(sb.len());
    sa.resize(len, 0);
    sb.resize(len, 0);
    Some(sa.cmp(&sb))
}

// ── 探测 ───────────────────────────────────────────────────

/// 运行给定命令，并从其标准输出 / 标准错误中解析版本号。
///
/// 与 `probe_version` 拆开，使「执行真实进程 → 解析输出」这一步可被独立验证。
fn read_version(cmd: &mut Command) -> Option<String> {
    #[cfg(windows)]
    {
        // 探测子进程不得弹出控制台窗口。
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    if let Some(v) = parse_version_output(&stdout) {
        return Some(v);
    }
    // 部分环境把 banner 写到 stderr，一并尝试。
    let stderr = String::from_utf8_lossy(&out.stderr);
    parse_version_output(&stderr)
}

/// 执行 `<bin> --version` 并解析版本号；文件缺失、不可执行或输出不可解析一律返回 `None`。
fn probe_version(bin: &Path) -> Option<String> {
    if !bin.is_file() {
        return None;
    }
    read_version(Command::new(bin).arg("--version"))
}

/// 目录是否存在且非空。
///
/// 目录名回退是弱信号，必须先确认目录里确实放了东西：目录不存在（全新机器）或为空
/// （仅由「打开目录」按钮创建的空壳）都不足以作为「已安装」的证据，否则会把
/// 「未安装」误报成与最佳适配版本一致。
fn dir_has_content(dir: &Path) -> bool {
    match std::fs::read_dir(dir) {
        Ok(mut entries) => entries.next().is_some(),
        Err(_) => false,
    }
}

/// 已安装版本探测（tasks 2.2 / 2.3）：优先执行二进制，失败回退解析目录名，仍失败返回 `None`。
pub fn detect_installed_version(dir: &Path) -> Option<String> {
    if let Some(v) = probe_version(&dir.join(SCRCPY_BIN)) {
        return Some(v);
    }
    if dir_has_content(dir) {
        return parse_version_from_dir_name(dir);
    }
    None
}

// ── 兼容性状态 ─────────────────────────────────────────────

/// 兼容性状态对象（design D1）。前端只负责渲染，不做版本比较。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatStatus {
    /// 最佳适配版本
    pub best_fit: String,
    /// 已安装版本（运行期探测；探测失败为 `unknown`）
    pub installed: String,
    /// "ok" | "mismatch"
    pub status: String,
    /// "equal" | "older" | "newer" | "unknown"
    pub direction: String,
}

/// 由「已安装版本」推导兼容性状态（design D1 / tasks 2.4）。
///
/// 四种取值：一致 → `ok`/`equal`；更旧 → `mismatch`/`older`；更新 → `mismatch`/`newer`；
/// 未知（未安装或无法探测）→ `mismatch`/`unknown`（界面给黄色叹号，但不阻断连接）。
pub fn build_compat_status(installed: Option<&str>) -> CompatStatus {
    let (status, direction, installed) = match installed {
        None => ("mismatch", "unknown", UNKNOWN_VERSION.to_string()),
        Some(v) => match compare_versions(v, BEST_FIT_VERSION) {
            Some(Ordering::Equal) => ("ok", "equal", v.to_string()),
            Some(Ordering::Less) => ("mismatch", "older", v.to_string()),
            Some(Ordering::Greater) => ("mismatch", "newer", v.to_string()),
            None => ("mismatch", "unknown", UNKNOWN_VERSION.to_string()),
        },
    };
    CompatStatus {
        best_fit: BEST_FIT_VERSION.to_string(),
        installed,
        status: status.to_string(),
        direction: direction.to_string(),
    }
}

// ── Tauri 命令 ─────────────────────────────────────────────

/// 兼容性状态命令（design D1 / D2，tasks 2.1–2.4）。
///
/// 标为 `async` 属性以便在独立线程执行（探测会拉起子进程），不阻塞 UI。
#[tauri::command(async)]
pub fn get_compat_status(app: AppHandle) -> Result<CompatStatus, String> {
    let dir = scrcpy_dir(&app)?;
    Ok(build_compat_status(detect_installed_version(&dir).as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// 真实 `scrcpy --version` 输出样例（main.c 的 banner + version.c 的依赖清单）。
    const REAL_OUTPUT: &str = "scrcpy 5.0.1 <https://github.com/Genymobile/scrcpy>\r\n\
                               \r\n\
                               Dependencies (compiled / linked):\r\n\
                               \x20- SDL: 3.4.18 / 3.4.18\r\n\
                               \x20- libavcodec: 62.11.100 / 62.11.100\r\n";

    fn unique_temp_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "fcn-test-{}-{}-{}",
            tag,
            std::process::id(),
            nanos
        ))
    }

    #[test]
    fn parses_version_from_real_output() {
        assert_eq!(
            parse_version_output(REAL_OUTPUT).as_deref(),
            Some(BEST_FIT_VERSION)
        );
    }

    #[test]
    fn parses_multi_segment_and_stderr_like_output() {
        assert_eq!(
            parse_version_output("scrcpy 3.1.1 <https://github.com/Genymobile/scrcpy>\n")
                .as_deref(),
            Some("3.1.1")
        );
        assert_eq!(parse_version_output("no banner here\n").as_deref(), None);
    }

    #[test]
    fn env_dir_follows_spec_layout() {
        // tasks 3.1：运行环境目录 = 应用数据根 + `scrcpy-{平台标签}-v{Version}`（带 `v`，
        // 与官方发布包同名），且为绝对路径。根路径按平台取 —— `C:\…` 在 Linux 上是**相对**路径，
        // 用它在非 Windows 上断言 `is_absolute()` 只会得到假失败。
        let root = Path::new(if cfg!(windows) {
            r"C:\Users\someone\AppData\Roaming\FreeControl2"
        } else {
            "/home/someone/.local/share/FreeControl2"
        });
        let dir = scrcpy_dir_at(root).unwrap();
        let platform = current().unwrap();

        assert_eq!(dir, root.join(dir_name_at(platform, BEST_FIT_VERSION)));
        assert!(dir.is_absolute());
        assert!(dir.starts_with(root));
        assert_eq!(dir.parent(), Some(root));
    }

    #[test]
    fn resolve_prefers_valid_custom_dir_and_falls_back() {
        let root = unique_temp_dir("resolve");
        let conventional = scrcpy_dir_at(&root).unwrap();

        // 未设置或空串 → 约定目录。
        assert_eq!(resolve_scrcpy_dir(&root, None).unwrap(), conventional);
        assert_eq!(resolve_scrcpy_dir(&root, Some("   ")).unwrap(), conventional);

        // 绝对路径 → 自定义目录（不要求已存在）。
        let custom = root.join("custom-env");
        assert_eq!(
            resolve_scrcpy_dir(&root, Some(custom.to_str().unwrap())).unwrap(),
            custom
        );

        // 相对路径 → 回落约定目录。
        assert_eq!(
            resolve_scrcpy_dir(&root, Some("relative/env")).unwrap(),
            conventional
        );

        // 指向既有文件 → 回落约定目录。
        fs::create_dir_all(&root).unwrap();
        let file = root.join("not-a-dir");
        fs::write(&file, b"x").unwrap();
        assert_eq!(
            resolve_scrcpy_dir(&root, Some(file.to_str().unwrap())).unwrap(),
            conventional
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn platform_table_is_two_dimensional() {
        // 平台标签由 OS × 架构两个维度共同决定；未知组合明确报错（MUST NOT 兜底猜一个标签）。
        let win = label_at("windows", "x86_64").unwrap();
        assert_eq!((win.tag, win.ext), ("win64", "zip"));

        let linux = label_at("linux", "x86_64").unwrap();
        assert_eq!((linux.tag, linux.ext), ("linux-x86_64", "tar.gz"));

        let err = label_at("linux", "aarch64").unwrap_err();
        assert!(
            err.contains("linux") && err.contains("aarch64"),
            "错误信息须含 OS 与架构，实际：{err}"
        );

        // 表外组合不得被「就近」映射到某个标签上：win32 与 macos 都不在支持范围。
        assert!(label_at("windows", "x86").is_err());
        assert!(label_at("macos", "aarch64").is_err());

        // 当前平台必须在表内：否则本应用在这个平台上既定不了目录名也定不了资产名。
        assert!(current().is_ok(), "当前平台不在受支持表内");
    }

    #[test]
    fn dir_name_follows_platform_tag() {
        // 目录名与平台标签同源；版本段取最佳适配版本常量 —— 升级时只改常量，不必改这里。
        let win = dir_name_at(label_at("windows", "x86_64").unwrap(), BEST_FIT_VERSION);
        assert!(win.starts_with("scrcpy-win64-v"), "实际：{win}");
        assert_eq!(win.strip_prefix("scrcpy-win64-v"), Some(BEST_FIT_VERSION));

        let linux = dir_name_at(label_at("linux", "x86_64").unwrap(), BEST_FIT_VERSION);
        assert!(linux.starts_with("scrcpy-linux-x86_64-v"), "实际：{linux}");
        assert_eq!(
            linux.strip_prefix("scrcpy-linux-x86_64-v"),
            Some(BEST_FIT_VERSION)
        );
    }

    #[test]
    fn windows_arm64_platform_row() {
        // 规格 `scrcpy-environment` 的 Windows ARM64 场景：官方标签 winarm64 + zip，
        // 目录名与 win64 各行同构（同一份下载、校验、解压与替换规则）。
        let arm = label_at("windows", "aarch64").unwrap();
        assert_eq!((arm.tag, arm.ext), ("winarm64", "zip"));
        assert_eq!(
            dir_name_at(arm, BEST_FIT_VERSION),
            format!("scrcpy-winarm64-v{BEST_FIT_VERSION}")
        );
    }

    #[test]
    fn arm64_dir_is_not_read_as_win64() {
        // 平台标签不得互相串读：ARM64 的目录不属于 win64 的运行环境（反之亦然）。
        let name = format!("/x/scrcpy-winarm64-v{BEST_FIT_VERSION}");
        assert_eq!(
            parse_version_from_dir_name_at(Path::new(&name), "win64"),
            None,
            "winarm64 目录不得被 win64 标签解析"
        );
        assert_eq!(
            parse_version_from_dir_name_at(Path::new(&name), "winarm64").as_deref(),
            Some(BEST_FIT_VERSION)
        );
        let win64 = format!("/x/scrcpy-win64-v{BEST_FIT_VERSION}");
        assert_eq!(
            parse_version_from_dir_name_at(Path::new(&win64), "winarm64"),
            None,
            "win64 目录不得被 winarm64 标签解析"
        );
    }

    #[test]
    fn parses_version_from_dir_name_both_spellings() {
        // 各平台标签都要能解析：带 / 不带 `v`、附加后缀，以及只有一个 `v` 的非法写法。
        for tag in ["win64", "winarm64", "linux-x86_64"] {
            let at = |name: &str| parse_version_from_dir_name_at(Path::new(name), tag);
            assert_eq!(
                at(&format!("/x/scrcpy-{tag}-v3.1")).as_deref(),
                Some("3.1"),
                "标签 {tag}"
            );
            assert_eq!(
                at(&format!("/x/scrcpy-{tag}-{BEST_FIT_VERSION}")).as_deref(),
                Some(BEST_FIT_VERSION),
                "标签 {tag}"
            );
            assert_eq!(
                at(&format!("/x/scrcpy-{tag}-v{BEST_FIT_VERSION}-portable")).as_deref(),
                Some(BEST_FIT_VERSION),
                "标签 {tag}"
            );
            assert_eq!(at(&format!("/x/scrcpy-{tag}-v")).as_deref(), None);
        }

        // 别的平台标签的目录不属于本平台（即使它存在于同一数据根下）。
        assert_eq!(
            parse_version_from_dir_name_at(Path::new("/x/scrcpy-win64-v3.1"), "linux-x86_64"),
            None
        );
        // 与任何平台标签都不相干的名字照旧解析不出来。
        assert_eq!(
            parse_version_from_dir_name(Path::new("/x/random-dir")).as_deref(),
            None
        );
    }

    #[test]
    fn compares_versions_numerically() {
        assert_eq!(compare_versions("4.1", "4.1"), Some(Ordering::Equal));
        assert_eq!(compare_versions("4.1", "4.1.0"), Some(Ordering::Equal));
        assert_eq!(compare_versions("4.0", "4.1"), Some(Ordering::Less));
        assert_eq!(compare_versions("4.10", "4.9"), Some(Ordering::Greater));
        assert_eq!(compare_versions("3.1", "4.1"), Some(Ordering::Less));
        assert_eq!(compare_versions("5.0", "4.1"), Some(Ordering::Greater));
        assert_eq!(compare_versions("abc", "4.1"), None);
    }

    #[test]
    fn compat_status_covers_four_cases() {
        let same = build_compat_status(Some(BEST_FIT_VERSION));
        assert_eq!((same.status.as_str(), same.direction.as_str()), ("ok", "equal"));

        let older = build_compat_status(Some("4.0"));
        assert_eq!(
            (older.status.as_str(), older.direction.as_str()),
            ("mismatch", "older")
        );

        let newer = build_compat_status(Some("6.2"));
        assert_eq!(
            (newer.status.as_str(), newer.direction.as_str()),
            ("mismatch", "newer")
        );

        let unknown = build_compat_status(None);
        assert_eq!(
            (unknown.status.as_str(), unknown.direction.as_str()),
            ("mismatch", "unknown")
        );
        assert_eq!(unknown.installed, UNKNOWN_VERSION);

        // bestFit 永远来自唯一常量。
        assert_eq!(same.best_fit, BEST_FIT_VERSION);
    }

    #[test]
    fn missing_binary_falls_back_to_dir_name() {
        // 目录内已有其它文件（如 adb.exe），仅缺 scrcpy.exe —— 回退到目录名。
        let dir = unique_temp_dir("fallback-missing");
        let target = dir.join(dir_name_at(current().unwrap(), "3.1"));
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("adb.exe"), b"stub").unwrap();

        let got = detect_installed_version(&target);
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(got.as_deref(), Some("3.1"));
    }

    #[test]
    fn absent_or_empty_dir_is_unknown() {
        // 全新机器：目录不存在 → unknown（不得用目录名回退出「与最佳适配一致」的假象）。
        let dir = unique_temp_dir("absent");
        let target = dir.join(dir_name_at(current().unwrap(), BEST_FIT_VERSION));
        assert_eq!(detect_installed_version(&target).as_deref(), None);

        // 空目录（仅由「打开目录」创建的空壳）同样不算已安装。
        fs::create_dir_all(&target).unwrap();
        let got = detect_installed_version(&target);
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(got.as_deref(), None);
    }

    #[test]
    #[cfg(windows)]
    fn unusable_binary_falls_back_to_dir_name() {
        // 「目录存在但二进制不可用于探测」：放一个真实可执行文件（where.exe）冒充 scrcpy，
        // 它不接受 `--version`，因此探测必须回退到目录名。
        let dir = unique_temp_dir("fallback-unusable");
        let target = dir.join(dir_name_at(current().unwrap(), "3.1"));
        fs::create_dir_all(&target).unwrap();
        let stub = target.join(SCRCPY_BIN);
        let system_exe = Path::new(r"C:\Windows\System32\where.exe");
        if system_exe.is_file() {
            fs::copy(system_exe, &stub).unwrap();
        } else {
            // 无系统样本可复制时退化为「文件存在但不可执行」的内容。
            fs::write(&stub, b"not a real executable").unwrap();
        }

        let got = detect_installed_version(&target);
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(got.as_deref(), Some("3.1"));
    }

    #[test]
    #[cfg(windows)]
    fn reads_version_from_a_real_process() {
        // 端到端验证「执行二进制 → 解析版本号」这一步（用 cmd /C type 冒充一个会打印版本行的进程）。
        let dir = unique_temp_dir("probe");
        fs::create_dir_all(&dir).unwrap();
        let out = dir.join("version.txt");
        fs::write(&out, REAL_OUTPUT).unwrap();

        let mut cmd = Command::new("cmd.exe");
        cmd.arg("/C").arg("type").arg(&out);
        let got = read_version(&mut cmd);

        let _ = fs::remove_dir_all(&dir);
        assert_eq!(got.as_deref(), Some(BEST_FIT_VERSION));
    }
}
