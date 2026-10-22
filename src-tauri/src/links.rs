//! 「关于」弹窗的三颗外部链接（about-page spec）。
//!
//! 分工与理由：
//! - **这里是全仓库唯一出现这三个地址的地方**。前端只传语义键（`repo` / `license` / `website`），
//!   既拿不到地址、也无法请求打开任意 URL —— 比「前端传 URL、后端校验 scheme + 主机白名单」更紧，
//!   也省掉一份与地址同源的校验清单。这与本仓既有的「事实在后端、前端只渲染」一致
//!   （scrcpy 版本、运行环境目录、GitHub 直连地址都是这么给的）。
//! - 打开动作交给**系统默认浏览器**（`shell::open_url`，与 `assets::open_scrcpy_dir` /
//!   `logger::open_log_dir` 同一手法）：应用自身不弹新窗口、不内嵌网页。
//! - 三个键都是 `https`：地址表由单测钉住（见文末），改地址时只改这一处。
//!
//! 「更新日志」已由应用内的「检查更新」取代（spec `about-page`），因此这里不再有 `changelog` 键。

use crate::shell;
use crate::ui_message::{codes, UserError};

/// 「关于」弹窗三颗外链入口的地址表 —— 全仓库唯一来源。
///
/// 许可指向仓库里的 `LICENSE` 原文，而不是在界面里内嵌一份许可副本 ——
/// 单 `.exe` 分发不带这个文件，能打开的就只有仓库那一份。
///
/// `website` 是本项目的官网落地页，与 README 里的官网链接同源（spec `project-website`
/// 「公开地址单一来源」）—— 换域名时这里与 README 要一起改。
///
/// `license` 落在仓库**默认分支**（`main`）上；若默认分支更名，这里要同步，
/// 否则界面上是一个 404 的死链。
pub const LINKS: [(&str, &str); 3] = [
    ("repo", "https://github.com/pdone/FreeControl2"),
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
pub fn open_external_link(link: String) -> Result<(), UserError> {
    let url = link_url(&link)
        .map_err(|e| UserError::new(codes::LINK_UNKNOWN).with_detail(e))?;
    shell::open_url(url)
        .map_err(|e| UserError::new(codes::LINK_OPEN_FAILED).with_detail(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 三个键各对应 spec 里写死的那个地址 —— 地址是界面契约的一部分，改错就等于跳错地方。
    #[test]
    fn every_key_maps_to_the_expected_url() {
        assert_eq!(
            link_url("repo").unwrap(),
            "https://github.com/pdone/FreeControl2"
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
    /// `support` / `changelog` 是**已撤下**的键：撤下后必须变成未知键 ——
    /// 若哪天又打开了，说明地址表里被塞回了界面上并不存在的入口。
    #[test]
    fn unknown_or_miscased_key_is_rejected_with_hint() {
        for bad in [
            "", "Website", "REPO", "docs", "website ", "support", "changelog", "about", "help",
        ] {
            let err = link_url(bad).unwrap_err();
            assert!(
                err.contains("repo") && err.contains("license") && err.contains("website"),
                "错误信息应列出全部可用键，实际：{err}"
            );
            let available = err.split("可用：").nth(1).unwrap_or("");
            assert!(
                !available.contains("changelog"),
                "更新日志入口已撤除，不应再出现在可用键里：{err}"
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
