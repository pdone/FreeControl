//! Linux 下按需关掉 WebKit 的 DMA-BUF 渲染器（本变更 design D12）。
//!
//! 问题：虚拟机 / 软件渲染 / 远程会话这类**没有可用 DMA-BUF 载体**的环境里，WebKit 默认的
//! DMA-BUF 渲染器这条帧传输链不成立 —— 表现是**首帧画得出来、之后界面不再更新**（点按钮其实
//! 生效，只是没被画出来）；`WEBKIT_DISABLE_DMABUF_RENDERER=1` 启动即正常。
//!
//! 判据取「有没有 DMA-BUF 载体」，**不取「是不是虚拟机」**：后者既不充分（真机驱动损坏、远程
//! 会话同样会坏）也不必要（虚拟机带 3D 加速时一切正常）。真正想问的「这条 GL 栈能不能可靠导入
//! dma-buf」在不创建 GL 上下文的前提下**不可观测**，所以只取两个可读代理，按短路次序：
//!
//! ```text
//! ① 用户已自行设过该变量        -> 让位（我们什么都不做）
//! ② DMI 命中已知虚拟化平台      -> 关
//! ③ 没有任何 /dev/dri/renderD*  -> 关（无渲染节点即无 GBM 载体）
//! ④ 都不命中                    -> 保持 WebKit 默认
//! ```
//!
//! 两头都会出错（漏判：真机驱动损坏 / XRDP、VNC / 嵌套虚拟化 / 未挂渲染节点的容器；误关：虚拟机
//! 但 GL 正常）。代价不对称 —— 漏判留给用户一个「点什么都没反应」且极难自查的窗口，而误关只是
//! 在 webview 上少一条加速路径（投屏画面在 scrcpy 自己的原生窗口里，webview 只有界面与文本），
//! 故取「命中即关」。
//!
//! **读输入与判定分离**：判定是不碰环境、不碰文件系统的纯函数，因此在 Windows 上也能被真单测
//! 覆盖 —— 本仓最大的盲区是「非宿主平台的分支本地编不到」，把决策形状抽出来是让它至少可断言。
//! 只有 `probe()` 那几行按平台门控（只在 Linux 编译：Windows 的 WebView2 与 macOS 的 WKWebView
//! 都不走 WebKitGTK 的这条帧传输，判定本体在它们的发布产物里不需要）。

/// WebKit 读的那个环境变量名。
///
/// 判定的产物就是「设 / 不设」这一个开关，**名字写错会静默失效**（不报错、只是不生效），
/// 故单测里连名字一起钉住。
#[cfg(any(target_os = "linux", test))]
pub const DISABLE_DMABUF_VAR: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";

/// 已知虚拟化平台的 DMI 厂商 / 机型关键字（design D12 的白名单）。
///
/// 取值是各家默认的 `sys_vendor` / `product_name`：VMware 是 "VMware, Inc." / "VMware Virtual
/// Platform"；VirtualBox 是 "innotek GmbH" / "VirtualBox"；QEMU-KVM 走 "QEMU" / "Standard PC"；
/// Hyper-V 的厂商串是真实的 "Microsoft Corporation"，只有机型名 "Virtual Machine" 能区分；
/// 另加 Bochs / Parallels / Xen。
///
/// 匹配是**大小写不敏感的子串**而不是精确相等：DMI 字串各版本略有出入（大小写、前后缀），
/// 精确相等会漏。**不要**把 "oracle" 单独加进来 —— Oracle 服务器是真实硬件厂商串，会误关。
#[cfg(any(target_os = "linux", test))]
const VIRTUALIZED_MARKERS: &[&str] = &[
    "vmware",
    "innotek",
    "virtualbox",
    "qemu",
    "kvm",
    "bochs",
    "parallels",
    "xen",
    "virtual machine",
];

/// 判定所需的全部输入 —— 由 `probe()` 或单测构造，判定本体不碰环境与文件系统。
#[cfg(any(target_os = "linux", test))]
#[derive(Debug)]
pub struct EnvProbe {
    /// `/sys/class/dmi/id/sys_vendor`；文件不存在或读不到（无权限）为 `None`。
    pub sys_vendor: Option<String>,
    /// `/sys/class/dmi/id/product_name`；同上。
    pub product_name: Option<String>,
    /// 是否存在任何 `/dev/dri/renderD*`（无渲染节点即无 GBM 载体）。
    pub has_render_node: bool,
    /// 用户是否**已自行设过**该变量（设成什么值都算设过 —— 我们只看「有没有」）。
    pub user_set_var: bool,
}

/// 判定结果，**带上是哪条判据命中的**。
///
/// 只回报一个布尔的话，把 ① 与 ② 调换也照样通过 —— 「命中了哪条」正是误关 / 漏判现场要看的
/// 东西，故它是结果的一部分，而不是另算一遍。
#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DmabufDecision {
    /// ① 用户已自行设过 → 让位，我们什么都不做。这也是漏判场景唯一的逃生门。
    DeferToUser,
    /// ② 命中已知虚拟化平台。
    Virtualized,
    /// ③ 没有任何 DRM 渲染节点。
    NoRenderNode,
    /// ④ 都不命中 → 保持 WebKit 默认。
    KeepDefault,
}

#[cfg(any(target_os = "linux", test))]
impl DmabufDecision {
    /// 是否要设那个环境变量。
    pub fn should_disable(self) -> bool {
        matches!(self, DmabufDecision::Virtualized | DmabufDecision::NoRenderNode)
    }

    /// 判据文案，供日志使用（四条互不相同，见单测）。
    pub fn reason(self) -> &'static str {
        match self {
            DmabufDecision::DeferToUser => "用户已自行设置该变量，让位",
            DmabufDecision::Virtualized => "命中已知虚拟化平台",
            DmabufDecision::NoRenderNode => "没有 DRM 渲染节点",
            DmabufDecision::KeepDefault => "未命中任何判据",
        }
    }
}

/// 纯判定：只看传入的输入，不碰环境、不碰文件系统（design D12 的短路次序）。
#[cfg(any(target_os = "linux", test))]
pub fn decide(probe: &EnvProbe) -> DmabufDecision {
    // ① 人工优先：用户设过就完全让位。
    if probe.user_set_var {
        return DmabufDecision::DeferToUser;
    }
    // ② DMI 厂商 / 机型命中白名单。读不到（None）不是中断，只是这条信号未命中。
    let looks_virtualized = [probe.sys_vendor.as_deref(), probe.product_name.as_deref()]
        .into_iter()
        .flatten()
        .any(|field| {
            let field = field.to_ascii_lowercase();
            VIRTUALIZED_MARKERS.iter().any(|marker| field.contains(marker))
        });
    if looks_virtualized {
        return DmabufDecision::Virtualized;
    }
    // ③ 无渲染节点即无 GBM 载体 —— 这一条覆盖「真机 / 容器但根本没有 DRM 节点」。
    // 它同时是 ② 读不到时的兜底判据，故放在其后。
    if !probe.has_render_node {
        return DmabufDecision::NoRenderNode;
    }
    // ④ 看上去是一条完整的硬件 GL 栈：不动 WebKit 的默认行为。
    DmabufDecision::KeepDefault
}

#[cfg(target_os = "linux")]
use crate::logger;
#[cfg(target_os = "linux")]
use std::sync::OnceLock;

/// 启动期算出的输入与决定，留给 `log_decision()` 落盘用。
#[cfg(target_os = "linux")]
static ACTIVE: OnceLock<(EnvProbe, DmabufDecision)> = OnceLock::new();

/// 读输入（Linux）：DMI 厂商 / 机型、有无 DRM 渲染节点、用户是否已设过变量。
///
/// 任何一步读失败（文件不存在 / 无权限 / 取值异常）都只是**该信号未命中**，接着看下一条 ——
/// 不得 panic、不得中断启动（design D12 的降级约束）。
#[cfg(target_os = "linux")]
fn probe() -> EnvProbe {
    use std::fs;

    fn read_field(path: &str) -> Option<String> {
        fs::read_to_string(path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    EnvProbe {
        sys_vendor: read_field("/sys/class/dmi/id/sys_vendor"),
        product_name: read_field("/sys/class/dmi/id/product_name"),
        // 目录不存在（无 GPU / 无 DRM）与「目录在但没有 renderD*」同样算无渲染节点。
        has_render_node: fs::read_dir("/dev/dri")
            .map(|entries| {
                entries
                    .flatten()
                    .any(|e| e.file_name().to_string_lossy().starts_with("renderD"))
            })
            .unwrap_or(false),
        user_set_var: std::env::var_os(DISABLE_DMABUF_VAR).is_some(),
    }
}

/// 启动期入口：判定并按需设变量。**必须早于 WebKit 初始化** —— 由 `main()` 在 `tauri::Builder`
/// 之前调用（窗口在 `setup` 之前创建、web 进程更晚，而 WebKit 是在自身初始化时读这个变量）。
///
/// 这里**只算不写日志**：此刻日志还没初始化，而 `logger::log` 在未初始化时静默丢弃
/// （`logger::init` 要到 `setup` 里、配置加载之后才跑，理由见归档变更 log-persistence 的 design
/// D12）。故把输入与决定留在 `ACTIVE` 里，由 `setup` 调 `log_decision()` 落盘。
#[cfg(target_os = "linux")]
pub fn prepare() {
    let probe = probe();
    let decision = decide(&probe);
    let _ = ACTIVE.set((probe, decision));
    if decision.should_disable() {
        // edition 2021 下 `set_var` 仍是安全函数；此刻是单线程启动期，无其他线程在读环境。
        std::env::set_var(DISABLE_DMABUF_VAR, "1");
    }
}

/// 非 Linux（Windows 的 WebView2、macOS 的 WKWebView）：不走 WebKitGTK 的 DMA-BUF 帧传输，无事可做。
#[cfg(not(target_os = "linux"))]
pub fn prepare() {}

/// 落盘判定行：`logger::init` 之后由 `setup` 调用（见 `prepare()` 的说明）。
///
/// 信号行与决定行分开：前者是**实测取值**，后者是**结论与命中的判据**。误关 / 漏判的现场只能
/// 靠这两行复盘（design D12 的留痕），故读不到的字段显式写成「读不到」，而不是省略。
#[cfg(target_os = "linux")]
pub fn log_decision() {
    let Some((probe, decision)) = ACTIVE.get() else {
        return;
    };
    logger::info(
        "app",
        format!(
            "WebKit 帧传输判定 —— sys_vendor={}，product_name={}，/dev/dri 渲染节点={}，用户已设该变量={}",
            probe.sys_vendor.as_deref().unwrap_or("<读不到>"),
            probe.product_name.as_deref().unwrap_or("<读不到>"),
            if probe.has_render_node { "有" } else { "无" },
            if probe.user_set_var { "是" } else { "否" },
        ),
    );
    logger::info(
        "app",
        format!(
            "WebKit DMA-BUF 渲染器{}（{}）",
            if decision.should_disable() { "已关闭" } else { "保持默认" },
            decision.reason()
        ),
    );
}

/// 非 Linux：没有这条判定可记。
#[cfg(not(target_os = "linux"))]
pub fn log_decision() {}

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一个输入；`sys_vendor` / `product_name` 传 `None` 表示「读不到」。
    fn probe(
        sys_vendor: Option<&str>,
        product_name: Option<&str>,
        has_render_node: bool,
        user_set_var: bool,
    ) -> EnvProbe {
        EnvProbe {
            sys_vendor: sys_vendor.map(str::to_string),
            product_name: product_name.map(str::to_string),
            has_render_node,
            user_set_var,
        }
    }

    /// 变量名写错 = 判定再对也静默失效，故连名字一起钉住。
    #[test]
    fn var_name_is_the_one_webkit_reads() {
        assert_eq!(DISABLE_DMABUF_VAR, "WEBKIT_DISABLE_DMABUF_RENDERER");
    }

    /// ① 人工优先：用户设过就让位 —— 哪怕同时命中 ②③。这是漏判场景唯一的逃生门。
    #[test]
    fn user_set_var_defers_even_when_other_criteria_hit() {
        let p = probe(
            Some("VMware, Inc."),
            Some("VMware Virtual Platform"),
            false,
            true,
        );
        assert_eq!(decide(&p), DmabufDecision::DeferToUser);
        assert!(!decide(&p).should_disable());
    }

    #[test]
    fn vmware_hits_virtualized() {
        let p = probe(
            Some("VMware, Inc."),
            Some("VMware Virtual Platform"),
            true,
            false,
        );
        assert_eq!(decide(&p), DmabufDecision::Virtualized);
    }

    #[test]
    fn virtualbox_hits_virtualized() {
        let p = probe(Some("innotek GmbH"), Some("VirtualBox"), true, false);
        assert_eq!(decide(&p), DmabufDecision::Virtualized);
    }

    /// Hyper-V 的厂商串是真实的 "Microsoft Corporation"，只有机型名能区分。
    #[test]
    fn hyper_v_hits_virtualized_via_product_name() {
        let p = probe(
            Some("Microsoft Corporation"),
            Some("Virtual Machine"),
            true,
            false,
        );
        assert_eq!(decide(&p), DmabufDecision::Virtualized);
    }

    /// 大小写与措辞各版本略有出入，故是大小写不敏感的子串匹配。
    #[test]
    fn marker_matching_is_case_insensitive() {
        let p = probe(Some("INNOTEK GMBH"), Some("VIRTUALBOX"), true, false);
        assert_eq!(decide(&p), DmabufDecision::Virtualized);
    }

    /// ④ 真机 + 有渲染节点 → 保持默认：不改正常机器的行为。
    #[test]
    fn physical_machine_with_render_node_keeps_default() {
        let p = probe(
            Some("ASUSTeK COMPUTER INC."),
            Some("Z170-PRO GAMING"),
            true,
            false,
        );
        assert_eq!(decide(&p), DmabufDecision::KeepDefault);
        assert!(!decide(&p).should_disable());
    }

    /// ③ 无渲染节点即无 GBM 载体 —— 覆盖「真机 / 容器但根本没有 DRM 节点」这一类。
    #[test]
    fn no_render_node_disables_even_when_not_virtualized() {
        let p = probe(
            Some("ASUSTeK COMPUTER INC."),
            Some("Z170-PRO GAMING"),
            false,
            false,
        );
        assert_eq!(decide(&p), DmabufDecision::NoRenderNode);
    }

    /// DMI 读不到（文件不存在 / 无权限）不是中断，而是**该信号未命中**，接着看 ③。
    #[test]
    fn unreadable_dmi_falls_through_to_render_node() {
        assert_eq!(
            decide(&probe(None, None, true, false)),
            DmabufDecision::KeepDefault
        );
        assert_eq!(
            decide(&probe(None, None, false, false)),
            DmabufDecision::NoRenderNode
        );
    }

    /// 四条判据的文案互不相同，且 `should_disable` 只对「要关」的两条为真 ——
    /// 日志里「命中了哪条」必须能分辨。
    #[test]
    fn every_criterion_reports_a_distinct_reason() {
        let all = [
            DmabufDecision::DeferToUser,
            DmabufDecision::Virtualized,
            DmabufDecision::NoRenderNode,
            DmabufDecision::KeepDefault,
        ];
        let mut reasons: Vec<&str> = all.iter().map(|d| d.reason()).collect();
        reasons.sort_unstable();
        reasons.dedup();
        assert_eq!(reasons.len(), all.len());
        assert!(DmabufDecision::Virtualized.should_disable());
        assert!(DmabufDecision::NoRenderNode.should_disable());
        assert!(!DmabufDecision::DeferToUser.should_disable());
        assert!(!DmabufDecision::KeepDefault.should_disable());
    }
}
