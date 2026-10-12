//! 「关于」弹窗的四个外部链接（about-page design D6）。
//!
//! 分工与理由：
//! - **这里是全仓库唯一出现这四个地址的地方**。前端只传语义键（`repo` / `changelog` / `license` /
//!   `website`），
//!   既拿不到地址、也无法请求打开任意 URL —— 比「前端传 URL、后端校验 scheme + 主机白名单」更紧，
//!   也省掉一份与地址同源的校验清单。这与本仓既有的「事实在后端、前端只渲染」一致
//!   （scrcpy 版本、运行环境目录、GitHub 直连地址都是这么给的）。
//! - 打开动作交给**系统默认浏览器**（`shell::open_url`，与 `assets::open_scrcpy_dir` /
//!   `logger::open_log_dir` 同一手法）：应用自身不弹新窗口、不内嵌网页。
//! - 四个键都是 `https`：地址表由单测钉住（见文末），改地址时只改这一处。

use crate::shell;

/// 「关于」弹窗四个外链入口的地址表 —— 全仓库唯一来源。
///
/// 前两颗（`repo` / `changelog`）是弹窗按钮行的第一排；`license` 与 `website` 是第二排
/// （spec「外链入口与打开方式」）。许可指向仓库里的 `LICENSE` 原文，而不是在界面里内嵌一份许可副本 ——
/// 单 `.exe` 分发不带这个文件，能打开的就只有仓库那一份。
///
/// `website` 是本项目的官网落地页，与 README 里的官网链接同源（spec `project-website`
/// 「公开地址单一来源」）—— 换域名时这里与 README 要一起改。
///
/// `changelog` 与 `license` 都落在仓库**默认分支**（`main`）上；若默认分支更名，这里要同步，
/// 否则界面上是两个 404 的死链。
pub const LINKS: [(&str, &str); 4] = [
    ("repo", "https://github.com/pdone/FreeControl2"),
    (
        "changelog",
        "https://github.com/pdone/FreeControl2/blob/main/Update.md",
    ),
    (
        "license",
        "https://github.com/pdone/FreeControl2/blob/main/LICENSE",
    ),
    ("website", "https://freecontrol2.pages.dev/"),
];

/// 由语义键取地址（纯函数，便于单测）。
///
/// 未知键一律拒绝，且错误信息里列出可用取值 —— 该信息直接用于界面提示（spec「外链入口与打开方式」：
/// 打开失败要给出可见反馈，不能静默失败）。
///
/// 键**区分大小写**：键由前端自己写，不做宽进严出 —— 随手拼错（`Website`）也能打开的话，
/// 真出错时反而更难发现。
pub fn link_url(link: &str) -> Result<&'static str, String> {
    LINKS
        .iter()
        .find(|(key, _)| *key == link)
        .map(|(_, url)| *url)
        .ok_or_else(|| {
            let keys = LINKS
                .iter()
                .map(|(key, _)| *key)
                .collect::<Vec<_>>()
                .join(" / ");
            format!("未知的链接标识「{link}」（可用：{keys}）")
        })
}

/// 用系统默认程序打开「关于」弹窗的某个外链（前端 `api.ts` 的 `openExternalLink`）。
///
/// 返回的 `Err` 直接用于界面提示：系统没有默认浏览器、调用被拒之类都在这里落成一句话，
/// 不让「点了没反应」发生。打开动作本身见 `shell`（Windows 上是一次 `ShellExecuteW`）。
#[tauri::command]
pub fn open_external_link(link: String) -> Result<(), String> {
    let url = link_url(&link)?;
    shell::open_url(url).map_err(|e| format!("打开链接失败：{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 四个键各对应 spec 里写死的那个地址 —— 地址是界面契约的一部分，改错就等于跳错地方。
    #[test]
    fn every_key_maps_to_the_expected_url() {
        assert_eq!(
            link_url("repo").unwrap(),
            "https://github.com/pdone/FreeControl2"
        );
        assert_eq!(
            link_url("changelog").unwrap(),
            "https://github.com/pdone/FreeControl2/blob/main/Update.md"
        );
        assert_eq!(
            link_url("license").unwrap(),
            "https://github.com/pdone/FreeControl2/blob/main/LICENSE"
        );
        assert_eq!(
            link_url("website").unwrap(),
            "https://freecontrol2.pages.dev/"
        );
    }

    /// 未知键与大小写变体一律拒绝，且错误信息里带上可用取值（界面直接展示这句话）。
    ///
    /// `support` 是**已撤下**的键：它撤下后必须变成未知键 ——
    /// 若哪天又打开了，说明地址表里被塞回了界面上并不存在的入口。
    #[test]
    fn unknown_or_miscased_key_is_rejected_with_hint() {
        for bad in [
            "", "Website", "REPO", "docs", "website ", "support", "Support", "about", "help",
        ] {
            let err = link_url(bad).unwrap_err();
            assert!(
                err.contains("repo")
                    && err.contains("changelog")
                    && err.contains("license")
                    && err.contains("website"),
                "错误信息应列出全部可用键，实际：{err}"
            );
        }
    }

    /// 全部地址必须是 https：明文 http 会被系统按不安全处理，也没有理由出现在这四处。
    #[test]
    fn every_address_is_https() {
        for (key, url) in LINKS {
            assert!(url.starts_with("https://"), "{key} 的地址不是 https：{url}");
        }
    }
}
