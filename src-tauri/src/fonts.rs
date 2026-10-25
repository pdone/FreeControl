//! 本机字体枚举（spec `app-preferences` 的「本机字体的枚举与过滤」）。
//!
//! 设置页的「字体」字段需要一份「本机已安装、适合界面使用」的字体族列表。
//! 枚举按平台实现：
//! - Windows：DirectWrite（与 WebView2 的字体集合同源；`windows` crate 已在依赖树里）；
//! - macOS / Linux：`fontdb`（纯 Rust，扫系统字体目录）。
//!
//! 两端都映射到同一个 [`SystemFont`]，再走共享的过滤、去重与排序 —— 平台差异只到
//! 「字体从哪里来」为止，配置、UI 与应用逻辑共用。

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

/// 一个可选的字体族。
///
/// `family` 是落盘与 CSS 使用的 canonical 族名（DirectWrite 优先 en-US 族名；
/// fontdb 把 en-US 族名排在首位）；`aliases` 保留全部语言的族名，供中文界面显示与搜索。
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SystemFont {
    pub family: String,
    pub aliases: Vec<String>,
    pub monospaced: bool,
}

/// 图标 / 符号 / emoji 字体：它们能装进 `font-family`，但用于界面只会得到一堆图形或空白。
/// 匹配族名的小写形式（包含即可），不按「好看 / 难看」过滤装饰字体。
const EXCLUDED_FAMILY_PATTERNS: &[&str] = &[
    "wingdings",
    "webdings",
    "marlett",
    "dingbat",
    "symbol",
    "emoji",
    "mdl2",
    "fluent icons",
    "material icons",
    "font awesome",
    "lastresort",
    "bookshelf",
    "hololens",
    "noto music",
    "noto sans symbols",
    "apple symbols",
    "segoe ui historic",
];

/// 族名是否适合作为界面字体。
///
/// `@` 是 Windows 上 CJK 字体的竖排别名，`.` 是 macOS 的私有 / 隐藏族名；
/// 空名与控制字符无法稳定写进 CSS，一并排除。
fn is_ui_suitable(family: &str) -> bool {
    let name = family.trim();
    if name.is_empty() || name.chars().any(char::is_control) {
        return false;
    }
    if name.starts_with('@') || name.starts_with('.') {
        return false;
    }
    let lower = name.to_lowercase();
    !EXCLUDED_FAMILY_PATTERNS
        .iter()
        .any(|pattern| lower.contains(pattern))
}

/// 去首尾空白、丢空项与控制字符、按不区分大小写去重，并保持首次出现的顺序。
fn dedupe_names(names: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    names
        .into_iter()
        .filter_map(|raw| {
            let name = raw.trim().to_string();
            if name.is_empty() || name.chars().any(char::is_control) {
                return None;
            }
            seen.insert(name.to_lowercase()).then_some(name)
        })
        .collect()
}

/// 把各字形的族名合并成「每族一项」。
///
/// 输入是每个字形（或 DirectWrite 的每个族）的族名列表与等宽标记；按 canonical 族名合并，
/// 等宽标记取并集，别名去重后保留。
fn collect_fonts(faces: impl IntoIterator<Item = (Vec<String>, bool)>) -> Vec<SystemFont> {
    let mut by_family: HashMap<String, SystemFont> = HashMap::new();

    for (raw_families, monospaced) in faces {
        let aliases = dedupe_names(raw_families);
        let Some(family) = aliases.first().cloned() else {
            continue;
        };
        if !is_ui_suitable(&family) {
            continue;
        }

        let key = family.to_lowercase();
        match by_family.get_mut(&key) {
            Some(existing) => {
                existing.monospaced |= monospaced;
                for alias in aliases {
                    if !existing
                        .aliases
                        .iter()
                        .any(|known| known.eq_ignore_ascii_case(&alias))
                    {
                        existing.aliases.push(alias);
                    }
                }
            }
            None => {
                by_family.insert(
                    key,
                    SystemFont {
                        family,
                        aliases,
                        monospaced,
                    },
                );
            }
        }
    }

    let mut fonts: Vec<SystemFont> = by_family.into_values().collect();
    fonts.sort_by(|a, b| a.family.to_lowercase().cmp(&b.family.to_lowercase()));
    fonts
}

#[cfg(not(windows))]
fn scan_system_fonts() -> Result<Vec<SystemFont>, String> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let faces = db.faces().map(|face| {
        (
            face.families
                .iter()
                .map(|(name, _)| name.clone())
                .collect::<Vec<_>>(),
            face.monospaced,
        )
    });
    Ok(collect_fonts(faces))
}

#[cfg(windows)]
mod directwrite;

#[cfg(windows)]
fn scan_system_fonts() -> Result<Vec<SystemFont>, String> {
    directwrite::scan()
}

/// 进程内缓存：字体在应用运行期间很少变化，设置页首次打开读一次即可。
/// 用户安装新字体后由菜单的刷新按钮传 `refresh = true` 重扫。
static CACHE: Mutex<Option<Vec<SystemFont>>> = Mutex::new(None);

/// 列出本机可用的字体族。
///
/// 扫描是阻塞 IO，放到 `spawn_blocking` 里，避免卡住事件循环。失败不写缓存，
/// 前端据此显示「无法读取本机字体」，并保留已保存字体的生效。
#[tauri::command]
pub async fn list_system_fonts(refresh: bool) -> Result<Vec<SystemFont>, String> {
    if !refresh {
        if let Some(cached) = CACHE.lock().unwrap().clone() {
            return Ok(cached);
        }
    }

    let fonts = match tauri::async_runtime::spawn_blocking(scan_system_fonts).await {
        Ok(Ok(fonts)) => fonts,
        Ok(Err(e)) => {
            crate::logger::error("fonts", format!("读取本机字体失败：{e}"));
            return Err(e);
        }
        Err(e) => {
            let message = format!("字体扫描任务失败：{e}");
            crate::logger::error("fonts", message.clone());
            return Err(message);
        }
    };

    crate::logger::info("fonts", format!("已读取 {} 个界面可用字体族", fonts.len()));
    *CACHE.lock().unwrap() = Some(fonts.clone());
    Ok(fonts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(families: &[&str], monospaced: bool) -> (Vec<String>, bool) {
        (
            families.iter().map(|name| (*name).to_string()).collect(),
            monospaced,
        )
    }

    #[test]
    fn excludes_icon_symbol_and_emoji_fonts() {
        let fonts = collect_fonts(vec![
            face(&["Wingdings"], false),
            face(&["Webdings"], false),
            face(&["Segoe UI Emoji"], false),
            face(&["Segoe MDL2 Assets"], false),
            face(&["Noto Color Emoji"], false),
            face(&["Noto Sans Symbols"], false),
            face(&["Apple Symbols"], false),
            face(&["Zapf Dingbats"], false),
            face(&["LastResort"], false),
            face(&["Segoe UI Historic"], false),
            face(&["Bookshelf Symbol 7"], false),
            face(&["HoloLens MDL2 Assets"], false),
            face(&["Noto Music"], false),
            face(&["Font Awesome 6 Free"], false),
            face(&["Material Icons"], false),
            face(&["Marlett"], false),
        ]);
        assert!(fonts.is_empty(), "图标 / 符号 / emoji 字体应全部排除");
    }

    #[test]
    fn excludes_vertical_aliases_private_names_and_empty_names() {
        let fonts = collect_fonts(vec![
            face(&["@SimSun"], false),
            face(&[".SF NS"], false),
            face(&["   "], false),
            face(&["Ari\nnal"], false),
            face(&["Arial"], false),
        ]);
        assert_eq!(fonts.len(), 1);
        assert_eq!(fonts[0].family, "Arial");
    }

    #[test]
    fn keeps_regular_families_and_monospaced_flag() {
        let fonts = collect_fonts(vec![
            face(&["Arial"], false),
            face(&["Cascadia Mono"], true),
            face(&["阿里巴巴普惠体 3.0"], false),
        ]);
        let names: Vec<&str> = fonts.iter().map(|f| f.family.as_str()).collect();
        assert_eq!(names, vec!["Arial", "Cascadia Mono", "阿里巴巴普惠体 3.0"]);
        assert!(fonts[1].monospaced);
        assert!(!fonts[0].monospaced);
    }

    #[test]
    fn merges_faces_of_the_same_family_and_keeps_aliases() {
        let fonts = collect_fonts(vec![
            face(&["Alibaba PuHuiTi 3.0", "阿里巴巴普惠体 3.0"], false),
            face(&["Alibaba PuHuiTi 3.0"], true),
        ]);
        assert_eq!(fonts.len(), 1);
        let font = &fonts[0];
        assert_eq!(font.family, "Alibaba PuHuiTi 3.0");
        assert_eq!(
            font.aliases,
            vec![
                "Alibaba PuHuiTi 3.0".to_string(),
                "阿里巴巴普惠体 3.0".to_string()
            ]
        );
        assert!(font.monospaced);
    }

    #[test]
    fn dedupes_aliases_case_insensitively() {
        let fonts = collect_fonts(vec![
            face(&["Cascadia Mono"], false),
            face(&["cascadia mono"], false),
        ]);
        assert_eq!(fonts.len(), 1);
        assert_eq!(fonts[0].family, "Cascadia Mono");
        assert_eq!(fonts[0].aliases, vec!["Cascadia Mono".to_string()]);
    }

    #[test]
    fn sorts_families_case_insensitively() {
        let fonts = collect_fonts(vec![
            face(&["zeta"], false),
            face(&["Alpha"], false),
            face(&["beta"], false),
        ]);
        let names: Vec<&str> = fonts.iter().map(|f| f.family.as_str()).collect();
        assert_eq!(names, vec!["Alpha", "beta", "zeta"]);
    }
}
