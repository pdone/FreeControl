//! 会话控制与多设备窗口排列（Group 9；design D6 / D11 / D13）。
//!
//! ## 会话 = 一个受本应用管理的 scrcpy 进程
//! `AppState.sessions` 以**设备 id** 为键保存会话记录（同一设备同时只允许一个会话）。记录同时
//! 承担三件事：
//! - **状态层**：`exit` 为 `None` 说明进程仍在运行，`device::merge_statuses` 据此判「已连接」
//!   （会话表优先于 adb 探测，见 design D13）；
//! - **槽位层**：`slot` 是窗口位置槽位，只有未退出的会话占用它（会话结束即释放，tasks 9.7）；
//! - **日志层**：`log` 保留该会话的输出；**进程退出后记录不删除** —— 异常退出后的输出正是
//!   排查依据（tasks 9.4），删掉就无从查看。仅保留最近 [`MAX_ENDED_SESSIONS`] 条已结束记录。
//!
//! ## 结束会话为什么不直接 `Child::kill`
//! `kill()` 是 `TerminateProcess`（硬杀），scrcpy 的清理逻辑（`--cleanup` 还原设备状态）会被跳过。
//! 故先走**优雅路径**：`taskkill` 不带 `/F` 向 scrcpy 的窗口发关闭请求，让它自己退出；超时才兜底强杀
//! （无窗口的 `--no-window` 会话收不到关闭请求，必然走强杀）。
//!
//! ## 窗口排列（tasks 9.5–9.9）
//! 只用 scrcpy 自身的 `--window-x` / `--window-y` 表达位置（design D6：不接管窗口句柄、零平台 API）。
//! 参数与几何计算全在纯函数里（[`columns`] / [`rows`] / [`slot_position`] / [`allocate_slot`]），
//! 便于用已知尺寸验证。三项约束：
//! 1. 行优先：先水平依次排列，一行排满后换行继续水平排列；
//! 2. 用户显式指定 `--window-x` / `--window-y` 时该会话不参与排列 —— 判断依据是**即将下发的参数
//!    本身**（[`plan_arrangement`]），而不是另一处状态，避免「界面以为让位了、实际没让」；
//! 3. 空间不足时复用槽位并提示可能重叠，**MUST NOT** 因此拒绝发起会话。

use crate::assets;
use crate::config;
use crate::config::Device;
use crate::device;
use crate::download::run_blocking;
use crate::logger;
use crate::scrcpy;
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};

/// 会话列表变化事件名（前端 `listen` 订阅；与 `device-status` 一样是「变化才推」）。
pub const EVENT_SESSION_CHANGED: &str = "session-changed";

/// 自动排列时工作区四周的留白（像素）。
pub const TILE_MARGIN: i32 = 8;
/// 自动排列时相邻窗口之间的间距（像素）。
pub const TILE_GAP: i32 = 8;
/// 窗口宽高未知时的经验默认值（design D6 实现要点：scrcpy 默认窗口尺寸取决于设备分辨率，
/// 应用侧无法精确预知，故「已知则用已知、未知则用经验默认」）。
pub const DEFAULT_WINDOW_W: i32 = 480;
/// 见 [`DEFAULT_WINDOW_W`]。
pub const DEFAULT_WINDOW_H: i32 = 900;
/// 单个会话保留的日志行数上限（留存要够排查，但不能无上限增长）。
pub const MAX_LOG_LINES: usize = 500;
/// 已结束会话最多保留多少条（log 留存用；更早的连同其记录一起丢弃）。
pub const MAX_ENDED_SESSIONS: usize = 20;
/// 优雅结束会话的最长等待时间（一般远小于它：scrcpy 收到关闭请求后立刻退出）。
pub const GRACEFUL_TIMEOUT: Duration = Duration::from_secs(3);
/// 等待进程退出的轮询步长。
const STOP_TICK: Duration = Duration::from_millis(50);

/// 拿不到任何显示器信息时的兜底工作区（仅 Windows 之外的平台 / 系统调用失败时才会用到）。
const FALLBACK_AREA: Area = Area {
    x: 0,
    y: 0,
    width: 1920,
    height: 1080,
};

// ── 工作区几何（纯函数，tasks 9.5 / 9.6） ──────────────────────

/// 矩形（工作区 / 窗口），单位像素。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// 主显示器**工作区**矩形（已扣除任务栏）。
///
/// 用 `SystemParametersInfoW(SPI_GETWORKAREA)` 而不是显示器分辨率：排列要在「不会压到任务栏」
/// 的范围内进行，否则最后一行的窗口会被任务栏盖住。
#[cfg(windows)]
pub fn primary_work_area() -> Option<Area> {
    use std::ffi::c_void;
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETWORKAREA};

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: 第三个参数是指向本函数栈上 `RECT` 的指针，调用期间一直有效；`uiparam` / `fwinini` 为 0。
    let ok = unsafe {
        SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut rect as *mut RECT as *mut c_void, 0)
    };
    if ok == 0 {
        return None;
    }
    Some(Area {
        x: rect.left,
        y: rect.top,
        width: rect.right - rect.left,
        height: rect.bottom - rect.top,
    })
}

/// 非 Windows 平台：取不到「扣除面板之后的可用工作区」—— 那需要 EWMH / GTK 等平台绑定
/// （design 的 Non-Goals 明确不引入）。故返回 `None`，由 [`work_area`] 退到「显示器区域扣除
/// [`panel_reserve`]」的保守取值。
#[cfg(not(windows))]
pub fn primary_work_area() -> Option<Area> {
    None
}

/// 窗口装饰（标题栏 / 边框）带来的额外尺寸，以及内容区相对窗口左上角的内缩。
///
/// 为什么需要：scrcpy 的 `--window-width` / `--window-height` 指的是**内容区**尺寸，
/// 而 SDL 把内容区定位到 `--window-x` / `--window-y` 指定的点上 —— Windows 的标题栏与边框
/// 会再向外扩张一圈（实测：内容区高 400 的窗口整体占 439 高）。排列时若按内容区尺寸算步长，
/// 相邻两行就会重叠（408 的步长压不住 439 的窗口），且首行的标题栏会跑到屏幕上方。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct FrameInsets {
    /// 内容区相对窗口左边缘的水平内缩（左/右边框）
    pub left: i32,
    /// 内容区相对窗口上边缘的垂直内缩（标题栏 + 上边框）
    pub top: i32,
    /// 窗口整体比内容区多出的宽度
    pub width: i32,
    /// 窗口整体比内容区多出的高度
    pub height: i32,
}

/// 系统当前窗口装饰的尺寸（非 Windows 平台返回全 0，v1 范围外）。
#[cfg(windows)]
pub fn window_frame_insets() -> FrameInsets {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXSIZEFRAME, SM_CXPADDEDBORDER, SM_CYCAPTION, SM_CYSIZEFRAME,
    };
    // SAFETY: 无参数、无副作用地读取系统度量值。
    let (frame_x, frame_y, caption, pad) = unsafe {
        (
            GetSystemMetrics(SM_CXSIZEFRAME),
            GetSystemMetrics(SM_CYSIZEFRAME),
            GetSystemMetrics(SM_CYCAPTION),
            GetSystemMetrics(SM_CXPADDEDBORDER),
        )
    };
    let left = frame_x + pad;
    let top = frame_y + pad + caption;
    FrameInsets {
        left,
        top,
        width: left * 2,
        height: top + frame_y + pad,
    }
}

/// 见 [`window_frame_insets`] 的 Windows 版本（design D4）。
///
/// 非 Windows 的取值有两点与 Windows 不同，都是有意为之：
/// 1. **`left` / `top` 为 0** —— Windows 上 SDL 定位的是窗口整体、装饰向外扩张，故要把内容
///    往右下推；而 X11 的窗口管理器会把客户区放到请求点上、装饰向外长，位置不需要内缩。
/// 2. **`width` / `height` 取保守常量** —— 取不到系统度量，只能按「足以盖住常见标题栏 +
///    边框」的量级计入步长与容量：宽度按两侧各 1px 的最小边框，高度沿用 Windows 侧的实测
///    量级（内容区高 400 的窗口整体占 439，即垂直装饰约 39）。宁可多留 8px 空隙，
///    也不要让相邻两行互相重叠。
#[cfg(not(windows))]
pub fn window_frame_insets() -> FrameInsets {
    FrameInsets {
        left: 0,
        top: 0,
        width: 2,
        height: 39,
    }
}

/// 非 Windows 平台上为主显示器顶部 / 底部系统占位（顶栏、dock）预留的像素（design D4）。
///
/// Windows 走的是「工作区」API（`SPI_GETWORKAREA`），任务栏已经在里面扣掉了，故为 `(0, 0)`；
/// 其它平台取不到「扣除面板之后的矩形」，只能以保守值兜底 —— 覆盖常见顶栏（约 32px）与
/// 常见 dock / 面板（≤56px）。宁可留白，也不要让最后一行被任务栏压住。
pub const fn panel_reserve() -> (i32, i32) {
    if cfg!(windows) {
        (0, 0)
    } else {
        (32, 56)
    }
}

/// 排列用的工作区：优先系统工作区，失败时退到「显示器区域扣除面板预留」，再失败给固定兜底值。
///
/// 回退链的顺序不变（design D6），变化只在「显示器区域」这一步：非 Windows 平台拿不到
/// 「扣除任务栏 / 面板之后的矩形」，故按 [`panel_reserve`] 扣掉一段保守占位。
pub fn work_area(app: &AppHandle) -> Area {
    if let Some(area) = primary_work_area() {
        return area;
    }
    if let Ok(Some(monitor)) = app.primary_monitor() {
        let size = monitor.size();
        let pos = monitor.position();
        let (top, bottom) = panel_reserve();
        // 高度最多扣到 1：极小屏幕上扣成负数会让后面的排列公式算出荒谬的容量。
        let height = (size.height as i32 - top - bottom).max(1);
        return Area {
            x: pos.x,
            y: pos.y + top,
            width: size.width as i32,
            height,
        };
    }
    FALLBACK_AREA
}

/// 每行可容纳的窗口数：`max(1, floor((work - 2*margin + gap) / (window + gap)))`（design D6）。
pub fn columns(work_w: i32, window_w: i32, margin: i32, gap: i32) -> usize {
    count_along(work_w, window_w, margin, gap)
}

/// 每列可容纳的窗口数，公式与 [`columns`] 同源（只是换成高度方向）。
pub fn rows(work_h: i32, window_h: i32, margin: i32, gap: i32) -> usize {
    count_along(work_h, window_h, margin, gap)
}

fn count_along(work: i32, window: i32, margin: i32, gap: i32) -> usize {
    let step = window + gap;
    if step <= 0 {
        return 1;
    }
    let n = (work - 2 * margin + gap) / step;
    if n < 1 {
        1
    } else {
        n as usize
    }
}

/// 槽位总数。
pub fn capacity(cols: usize, rows: usize) -> usize {
    cols.max(1) * rows.max(1)
}

/// 槽位 → 左上角坐标（**行优先**：先水平依次排列，一行排满后换行继续水平排列）。
pub fn slot_position(
    slot: usize,
    cols: usize,
    area: &Area,
    window_w: i32,
    window_h: i32,
    margin: i32,
    gap: i32,
) -> (i32, i32) {
    let cols = cols.max(1);
    let col = (slot % cols) as i32;
    let row = (slot / cols) as i32;
    (
        area.x + margin + col * (window_w + gap),
        area.y + margin + row * (window_h + gap),
    )
}

/// 分配槽位：取**最小的空闲索引**；全部占用时复用最小已占用索引并报告降级
/// （tasks 9.7 释放后复用 / 9.8 空间不足降级）。
pub fn allocate_slot(occupied: &BTreeSet<usize>, capacity: usize) -> (usize, bool) {
    let cap = capacity.max(1);
    for slot in 0..cap {
        if !occupied.contains(&slot) {
            return (slot, false);
        }
    }
    (occupied.iter().next().copied().unwrap_or(0), true)
}

// ── 参数解析（判断用户是否已指定位置 / 窗口尺寸） ──────────────

/// 把参数摊平成 token：自定义参数是一整条字符串（如 `--window-x=100 --window-y=50`），
/// 按空白拆开才能与其它 flag 一视同仁地识别。
fn flat_tokens(args: &[String]) -> Vec<String> {
    args.iter()
        .flat_map(|a| a.split_whitespace().map(|t| t.to_string()))
        .collect()
}

/// 参数里是否出现了某个长选项（`--name=value` 或 `--name value`）。
fn has_flag(tokens: &[String], name: &str) -> bool {
    tokens
        .iter()
        .any(|t| t == name || t.starts_with(&format!("{name}=")))
}

/// 取长选项的整数值（`--name=800` / `--name 800`）；未出现或不可解析时为 `None`。
fn int_flag(tokens: &[String], name: &str) -> Option<i32> {
    let prefix = format!("{name}=");
    for (i, t) in tokens.iter().enumerate() {
        if let Some(rest) = t.strip_prefix(&prefix) {
            return rest.parse::<i32>().ok();
        }
        if t == name {
            return tokens.get(i + 1).and_then(|v| v.parse::<i32>().ok());
        }
    }
    None
}

/// 取长选项的字符串值（`--name=value` / `--name value`）。
fn flag_value(tokens: &[String], name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    for (i, t) in tokens.iter().enumerate() {
        if let Some(rest) = t.strip_prefix(&prefix) {
            return Some(rest.to_string());
        }
        if t == name {
            return tokens.get(i + 1).cloned();
        }
    }
    None
}

// ── 窗口尺寸估算（tasks 9.5–9.6 的前提） ───────────────────────
//
// scrcpy 的 `--window-width` / `--window-height` 是**上限**，不是最终尺寸：
// `get_initial_optimal_size()`（`doc/scrcpy/app/src/screen.c`）先取请求值，随后**默认开启**的
// 宽高比锁（`--window-aspect-ratio-lock`）按视频宽高比把它收窄 —— 请求 480x400 的手机竖屏
// 窗口实际只有约 185x400。排列若按请求值算步长，两窗之间会空出比窗口本身还宽的缝隙，
// 并且会**过早**判定「空间不足」而去复用槽位。故这里按设备显示尺寸推算出真实的内容区尺寸。

/// 从 `dumpsys display` 输出里解析主显示的**当前**尺寸（像素）。
///
/// 取 `mBaseDisplayInfo` 行的 `real W x H`（scrcpy 用的是显示的真实尺寸，不是扣除导航栏的
/// `app` 区域 —— 实测内容宽高比与 `real 1080 x 2340` 一致），并按 `rotation` 换算成实际方向：
/// 1 / 3 为横屏，宽高互换。解析不出来时返回 `None`（调用方退回按请求值估算）。
pub fn parse_display_size(output: &str) -> Option<(i32, i32)> {
    let line = output
        .lines()
        .find(|l| l.contains("mBaseDisplayInfo"))
        .or_else(|| output.lines().find(|l| l.contains("real ")))?;
    // 数值后面紧跟逗号（`real 1080 x 2340, largest app …`），故只取前导数字。
    let leading_number = |word: &str| -> Option<i32> {
        let digits: String = word.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            None
        } else {
            digits.parse::<i32>().ok()
        }
    };
    let rest = line.split("real ").nth(1)?;
    let mut words = rest.split_whitespace();
    let width = leading_number(words.next()?)?;
    if words.next()? != "x" {
        return None;
    }
    let height = leading_number(words.next()?)?;
    if width <= 0 || height <= 0 {
        return None;
    }
    let rotated = line
        .split("rotation ")
        .nth(1)
        .and_then(|r| {
            r.split(|c: char| !c.is_ascii_digit())
                .next()
                .filter(|d| !d.is_empty())
                .and_then(|d| d.parse::<u32>().ok())
        })
        .map(|r| r == 1 || r == 3)
        .unwrap_or(false);
    Some(if rotated {
        (height, width)
    } else {
        (width, height)
    })
}

/// 查询设备当前显示尺寸；失败（无 adb / 超时 / 老系统无该字段 / 设备侧无 grep）返回 `None`。
///
/// **过滤必须放在设备侧**：`dumpsys display` 的输出有上百 KB，而 [`device::run_captured`] 要等
/// 子进程退出后才读管道 —— 输出写满管道缓冲时子进程会阻塞，最终被超时误杀并返回错误
/// （实测：未过滤时永远拿不到结果，加上过滤后 0.8 秒返回）。
fn query_display_size(adb: &Path, connection_id: &str) -> Option<(i32, i32)> {
    let out = device::run_captured(
        adb,
        &[
            "-s",
            connection_id,
            "shell",
            "dumpsys display | grep -m1 mBaseDisplayInfo",
        ],
        device::PROBE_TIMEOUT,
    )
    .ok()?;
    parse_display_size(&out)
}

/// 本次会话的**视频内容尺寸**；无法预知时为 `None`。
///
/// 与设备分辨率无关的模式一律返回 `None`：摄像头、Flex Display、非 0 的 `--display-id`。
/// 其余按 scrcpy 的参数语义依次施加 `--new-display` / `--crop` / `--angle` / `--max-size`。
fn content_size(tokens: &[String], display: Option<(i32, i32)>) -> Option<(i32, i32)> {
    if flag_value(tokens, "--video-source").as_deref() == Some("camera") {
        return None;
    }
    if has_flag(tokens, "--flex-display") {
        return None;
    }
    if int_flag(tokens, "--display-id").is_some_and(|id| id != 0) {
        return None;
    }

    // 虚拟显示器：`--new-display=WxH[/DPI]`；只写 `--new-display` 时仍以主屏尺寸为内容尺寸。
    let mut size = match flag_value(tokens, "--new-display") {
        Some(spec) => {
            let dims = spec.split('/').next().unwrap_or("");
            let mut parts = dims.split(['x', 'X']);
            let w = parts.next().and_then(|s| s.trim().parse::<i32>().ok());
            let h = parts.next().and_then(|s| s.trim().parse::<i32>().ok());
            match (w, h) {
                (Some(w), Some(h)) if w > 0 && h > 0 => (w, h),
                _ => display?,
            }
        }
        None => display?,
    };

    // `--crop=W:H:X:Y` 直接给定内容尺寸。
    if let Some(crop) = flag_value(tokens, "--crop") {
        let mut parts = crop.split(':');
        let w = parts.next().and_then(|s| s.trim().parse::<i32>().ok());
        let h = parts.next().and_then(|s| s.trim().parse::<i32>().ok());
        if let (Some(w), Some(h)) = (w, h) {
            if w > 0 && h > 0 {
                size = (w, h);
            }
        }
    }

    // `--angle=90|270` 把画面转置，宽高互换。
    if matches!(flag_value(tokens, "--angle").as_deref(), Some("90") | Some("270")) {
        size = (size.1, size.0);
    }

    // `--max-size=N`（前端用短选项 `-m N`）把长边限到 N。
    if let Some(max) = int_flag(tokens, "--max-size").or_else(|| int_flag(tokens, "-m")) {
        let longest = size.0.max(size.1);
        if max > 0 && longest > max {
            let scale = max as f64 / longest as f64;
            size = (
                ((size.0 as f64 * scale).round() as i32).max(1),
                ((size.1 as f64 * scale).round() as i32).max(1),
            );
        }
    }

    Some(size)
}

/// 把内容尺寸按宽高比缩到给定边界内（不放大）。
fn fit_into(content: (i32, i32), bounds: (i32, i32)) -> (i32, i32) {
    let (cw, ch) = content;
    let (bw, bh) = bounds;
    if bw <= 0 || bh <= 0 {
        return content;
    }
    if cw <= bw && ch <= bh {
        return content;
    }
    let scale = (bw as f64 / cw as f64).min(bh as f64 / ch as f64);
    (
        ((cw as f64 * scale).round() as i32).max(1),
        ((ch as f64 * scale).round() as i32).max(1),
    )
}

/// 由内容尺寸与用户请求的窗口尺寸，算出 scrcpy 实际会开出的窗口**内容区**尺寸。
///
/// 对应 scrcpy 的 `get_initial_optimal_size` + 宽高比锁：
/// - 两个请求值都给 → 把请求框按内容宽高比收成「不超出框」的最大同比例尺寸；
/// - 只给一个 → 另一个按内容宽高比推导；
/// - 都不给 → 内容尺寸按宽高比缩进边界内（`get_optimal_size(content, content, true)`，不放大）。
fn fit_content(content: (i32, i32), req_w: i32, req_h: i32, bounds: (i32, i32)) -> (i32, i32) {
    let (cw, ch) = content;
    if cw <= 0 || ch <= 0 {
        return (
            if req_w > 0 { req_w } else { DEFAULT_WINDOW_W },
            if req_h > 0 { req_h } else { DEFAULT_WINDOW_H },
        );
    }
    let aspect = cw as f64 / ch as f64;
    let (mut w, mut h) = match (req_w > 0, req_h > 0) {
        (true, true) => (req_w, req_h),
        (true, false) => (req_w, ((req_w as f64 / aspect).round() as i32).max(1)),
        (false, true) => (((req_h as f64 * aspect).round() as i32).max(1), req_h),
        (false, false) => return fit_into(content, bounds),
    };
    // 宽高比锁：请求框与内容比例不符时按「不超出框」收窄（这正是实测 480x400 → 185x400 的成因）。
    // 两个方向各自取小，且**只在更小时才改**：这样对已经成比例的尺寸是幂等的（不因四舍五入反复调整）。
    let width_from_height = (((h as f64) * aspect).round() as i32).max(1);
    if width_from_height < w {
        w = width_from_height;
    }
    let height_from_width = (((w as f64) / aspect).round() as i32).max(1);
    if height_from_width < h {
        h = height_from_width;
    }
    (w.max(1), h.max(1))
}

/// 本次会话窗口**内容区**尺寸：能推算视频尺寸时按 scrcpy 的规则适配请求框；
/// 推算不出来时退回「请求值，未指定则用经验默认值」——即改造前的行为，不因取不到尺寸而变差。
fn window_content_size(
    tokens: &[String],
    display: Option<(i32, i32)>,
    bounds: (i32, i32),
) -> (i32, i32) {
    let req_w = int_flag(tokens, "--window-width").unwrap_or(0);
    let req_h = int_flag(tokens, "--window-height").unwrap_or(0);
    if let Some(content) = content_size(tokens, display) {
        return fit_content(content, req_w, req_h, bounds);
    }
    (
        if req_w > 0 { req_w } else { DEFAULT_WINDOW_W },
        if req_h > 0 { req_h } else { DEFAULT_WINDOW_H },
    )
}

// ── 排列计划 ───────────────────────────────────────────────────

/// 为什么一次会话没有参与自动窗口排列。
///
/// 单独成枚举而不塞进布尔值：两种原因在界面上的提示完全不同 —— `User` 是用户自己的选择，
/// `Platform` 则必须明确告知「窗口位置由系统决定、可能重叠」，否则用户会以为排列坏了
/// （design D5）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ArrangeSkipReason {
    /// 用户显式指定了 `--window-x` / `--window-y`
    User,
    /// 显示协议不允许应用指定窗口位置（如 Linux 的 Wayland 会话）
    Platform,
}

/// 一次会话的排列结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arrangement {
    /// 是否参与自动排列（用户已指定位置时为 `false`）
    pub arranged: bool,
    /// 未参与自动排列的原因（`arranged` 为 `true` 时为 `None`）
    pub opt_out: Option<ArrangeSkipReason>,
    pub slot: Option<usize>,
    pub x: Option<i32>,
    pub y: Option<i32>,
    /// 该会话复用了已占用的槽位（空间不足，窗口可能重叠）
    pub reused: bool,
}

impl Arrangement {
    fn opt_out(reason: ArrangeSkipReason) -> Self {
        Arrangement {
            arranged: false,
            opt_out: Some(reason),
            slot: None,
            x: None,
            y: None,
            reused: false,
        }
    }
}

/// 当前显示协议是否允许应用指定窗口位置（design D5）。
///
/// Wayland（以及一切把定位权交给合成器的协议）下 scrcpy 只能调 `SDL_SetWindowPosition`
/// （`doc/scrcpy/app/src/util/sdl.c`），而后者会被合成器无视且只记一行日志 —— 此时「自动排列」
/// 是假的：窗口全都叠在合成器给出的位置上，却没有任何解释。判据用环境变量：
/// `XDG_SESSION_TYPE=wayland` 或 `WAYLAND_DISPLAY` 非空即判为不可定位。
/// 做成接参数的纯函数，便于在任一平台上验证两种输入。
pub fn can_position_windows(session_type: Option<&str>, wayland_display: Option<&str>) -> bool {
    let wayland = session_type
        .map(|v| v.trim().eq_ignore_ascii_case("wayland"))
        .unwrap_or(false)
        || wayland_display.map(|v| !v.trim().is_empty()).unwrap_or(false);
    !wayland
}

/// 读环境变量判定本会话能否定位窗口。
fn can_position_now() -> bool {
    can_position_windows(
        std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
    )
}

/// 决定本次会话的窗口位置（tasks 9.6 / 9.8 / 9.9）。
///
/// 窗口尺寸取「scrcpy 实际会开出的内容区尺寸」（见 [`window_content_size`]：按设备显示尺寸与
/// 宽高比适配请求框），`display` 是设备当前显示尺寸（取不到时传 `None`，退回按请求值估算）。
///
/// 内容区尺寸之外还要加上窗口装饰：scrcpy / SDL 定位与设尺寸用的都是**内容区**，
/// 网格按「内容区 + 装饰」算、返回的坐标再让出装饰的内缩 —— 否则相邻行重叠、首行标题栏越出屏幕。
///
/// 本函数只管「**能定位窗口时**怎么排」；显示协议不允许定位（如 Wayland 会话）时由调用方
/// 直接给 `Arrangement::opt_out(ArrangeSkipReason::Platform)`，不进本函数 —— 那时连
/// 「算出位置却发不出去」都不该发生，更不该为此多跑一次显示尺寸查询。
pub fn plan_arrangement(
    occupied: &BTreeSet<usize>,
    area: &Area,
    args: &[String],
    insets: FrameInsets,
    display: Option<(i32, i32)>,
) -> Arrangement {
    let tokens = flat_tokens(args);
    if has_flag(&tokens, "--window-x") || has_flag(&tokens, "--window-y") {
        return Arrangement::opt_out(ArrangeSkipReason::User);
    }
    let bounds = (area.width - 2 * TILE_MARGIN, area.height - 2 * TILE_MARGIN);
    let (content_w, content_h) = window_content_size(&tokens, display, bounds);
    let win_w = content_w + insets.width;
    let win_h = content_h + insets.height;
    let cols = columns(area.width, win_w, TILE_MARGIN, TILE_GAP);
    let rows = rows(area.height, win_h, TILE_MARGIN, TILE_GAP);
    let (slot, reused) = allocate_slot(occupied, capacity(cols, rows));
    let (frame_x, frame_y) = slot_position(slot, cols, area, win_w, win_h, TILE_MARGIN, TILE_GAP);
    Arrangement {
        arranged: true,
        opt_out: None,
        slot: Some(slot),
        x: Some(frame_x + insets.left),
        y: Some(frame_y + insets.top),
        reused,
    }
}

/// 槽位复用时的用户提示（tasks 9.8：允许复用但要说清楚）。
pub fn degraded_warning() -> String {
    "屏幕空间不足以容纳全部窗口：已复用窗口位置，部分投屏窗口可能相互重叠。\
     可在「窗口」页调小 --window-width / --window-height，或为该设备指定 --window-x / --window-y。"
        .to_string()
}

// ── 会话记录 ───────────────────────────────────────────────────

/// 一个会话（一台设备一条）。
#[derive(Clone)]
pub struct SessionRecord {
    pub device_id: String,
    pub alias: String,
    pub connection_id: String,
    pub pid: u32,
    /// 启动时刻（Unix 毫秒）
    pub started_at: u64,
    /// 实际执行的命令（形如 `scrcpy -s <标识> <参数…>`，与命令预览同构）
    pub command: String,
    pub arranged: bool,
    /// 未参与自动排列的原因（design D5）
    pub arrange_skip: Option<ArrangeSkipReason>,
    pub slot: Option<usize>,
    pub window_x: Option<i32>,
    pub window_y: Option<i32>,
    pub slot_reused: bool,
    log: Arc<Mutex<Vec<String>>>,
    child: Arc<Mutex<Child>>,
    /// `None` = 仍在运行；`Some(code)` = 已退出（退出码不可得时为 `-1`）
    exit: Arc<Mutex<Option<i32>>>,
}

impl SessionRecord {
    fn running(&self) -> bool {
        self.exit.lock().unwrap().is_none()
    }

    fn push_log(&self, line: impl Into<String>) {
        self.push_log_at(logger::LEVEL_INFO, line.into());
    }

    /// 指定级别记一行（同一个 `push_log` 出口：内存缓冲与日志文件收到的是同一份过滤结果）。
    fn push_log_at(&self, level: &str, line: String) {
        push_log(&self.log, &log_source(&self.alias), level, line);
    }

    fn info(&self) -> SessionInfo {
        let exit = *self.exit.lock().unwrap();
        let log_lines = self.log.lock().unwrap().len();
        SessionInfo {
            device_id: self.device_id.clone(),
            alias: self.alias.clone(),
            connection_id: self.connection_id.clone(),
            pid: self.pid,
            started_at: self.started_at,
            command: self.command.clone(),
            arranged: self.arranged,
            arrange_skip: self.arrange_skip,
            slot: self.slot,
            window_x: self.window_x,
            window_y: self.window_y,
            slot_reused: self.slot_reused,
            running: exit.is_none(),
            exit_code: exit,
            log_lines,
        }
    }
}

/// 下发给前端的会话信息。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub device_id: String,
    pub alias: String,
    pub connection_id: String,
    pub pid: u32,
    pub started_at: u64,
    pub command: String,
    /// 是否参与自动窗口排列
    pub arranged: bool,
    /// 未参与自动排列的原因（参与时为 `None`；前端据此选提示文案，design D5）
    pub arrange_skip: Option<ArrangeSkipReason>,
    pub slot: Option<usize>,
    pub window_x: Option<i32>,
    pub window_y: Option<i32>,
    /// 空间不足而复用了已占用的槽位（窗口可能与其他窗口重叠）
    pub slot_reused: bool,
    /// 进程是否仍在运行（已结束的会话保留其日志供排查，tasks 9.4）
    pub running: bool,
    pub exit_code: Option<i32>,
    pub log_lines: usize,
}

/// 单台设备的连接请求：参数由前端拼装（字段模型归前端所有，见 `src/lib/params.ts`），
/// 后端只负责补 `-s <连接标识>` 与自动排列的窗口位置。
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConnectRequest {
    pub device_id: String,
    pub args: Vec<String>,
}

/// 单台设备的连接结果（批量时逐台回传，便于界面把失败原因贴到对应设备上）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConnectOutcome {
    pub device_id: String,
    pub alias: String,
    pub ok: bool,
    pub error: Option<String>,
}

/// 连接（单台或批量）的结果。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConnectResult {
    pub outcomes: Vec<ConnectOutcome>,
    pub sessions: Vec<SessionInfo>,
    /// 本次排列发生了槽位复用
    pub degraded: bool,
    pub warning: Option<String>,
}

// ── 状态读取（各自短暂加锁，不嵌套） ───────────────────────────

fn find_device(state: &AppState, id: &str) -> Result<Device, String> {
    state
        .config
        .lock()
        .unwrap()
        .devices
        .iter()
        .find(|d| d.id == id)
        .cloned()
        .ok_or_else(|| "设备不存在".to_string())
}

/// 某台设备的活跃会话记录（已退出的不算）。
pub fn active_record(state: &AppState, device_id: &str) -> Option<SessionRecord> {
    let map = state.sessions.lock().unwrap();
    map.get(device_id)
        .filter(|r| r.running())
        .cloned()
}

/// 仍在运行的会话所对应的设备 id 集合（`device::merge_statuses` 的「会话层」输入）。
///
/// 已结束的会话**不算**：它的记录还留在表里只是为了查看日志（tasks 9.4），设备状态必须回到
/// 空闲（界面上的「在线」）/ 离线，否则会出现「进程早已退出、界面还显示已连接」。
pub fn active_ids(state: &AppState) -> HashSet<String> {
    records(state)
        .into_iter()
        .filter(|r| r.running())
        .map(|r| r.device_id)
        .collect()
}

/// 全部会话记录（含已结束的，供日志查看）。
fn records(state: &AppState) -> Vec<SessionRecord> {
    state.sessions.lock().unwrap().values().cloned().collect()
}

/// 仍被占用的槽位。
pub fn occupied_slots(state: &AppState) -> BTreeSet<usize> {
    records(state)
        .into_iter()
        .filter(|r| r.running())
        .filter_map(|r| r.slot)
        .collect()
}

/// 会话列表（最近启动的排前面）。
pub fn session_infos(state: &AppState) -> Vec<SessionInfo> {
    let mut list: Vec<SessionInfo> = records(state).iter().map(SessionRecord::info).collect();
    list.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    list
}

/// 清空某一条会话已保留的输出（「日志」页的「清空」按钮）。
///
/// 只清 [`SessionRecord::log`] 缓冲：记录本身、退出码与实际命令一律留着 —— 清掉输出不等于删掉
/// 会话，其它会话的输出也不受牵连。参数取会话表而不是 `&AppState`，是为了让「查得到 / 查不到 /
/// 只清这一条」这三件事可被单测覆盖（[`session_infos`] 那条路径同理）。
fn clear_log(
    sessions: &Mutex<HashMap<String, SessionRecord>>,
    device_id: &str,
) -> Result<(), String> {
    let record = sessions
        .lock()
        .unwrap()
        .get(device_id)
        .cloned()
        .ok_or_else(|| "该设备没有会话记录".to_string())?;
    record.log.lock().unwrap().clear();
    Ok(())
}

/// 推送会话列表（变化才推；`list_sessions` 命令始终可取出实时值，故不需要缓存快照）。
pub fn publish_sessions(app: &AppHandle, infos: &[SessionInfo]) {
    let _ = app.emit(EVENT_SESSION_CHANGED, infos.to_vec());
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ── 进程与日志 ─────────────────────────────────────────────────

/// 一次会话真正下发的 argv：`-s <连接标识>` + 前端参数 + 自动排列写入的位置参数。
///
/// 单独抽成纯函数，是为了让「命令行文本」与「实际 argv」只有一份构造逻辑：
/// 两者各自拼一遍 `-s <标识>` 时，显示串里就会出现两条 `-s`（曾发生）。
fn session_argv(connection_id: &str, args: &[String], plan: &Arrangement) -> Vec<String> {
    let mut full: Vec<String> = vec!["-s".to_string(), connection_id.to_string()];
    full.extend(args.iter().cloned());
    if let (Some(x), Some(y)) = (plan.x, plan.y) {
        full.push(format!("--window-x={x}"));
        full.push(format!("--window-y={y}"));
    }
    full
}

/// 命令行文本（与命令预览同构，便于用户对照「预览的」与「实际执行的」）。
///
/// **入参就是真正传给 scrcpy 的 argv**，只在前面补可执行文件名 —— 不在这里另拼 `-s`，
/// 否则与 `session_argv` 各拼一次，界面上会显示成 `scrcpy -s X -s X …`。
fn command_line(argv: &[String]) -> String {
    let mut parts = vec!["scrcpy".to_string()];
    parts.extend(argv.iter().cloned());
    parts.join(" ")
}

fn exit_code(status: ExitStatus) -> i32 {
    status.code().unwrap_or(-1)
}

/// 子进程的 `PATH`：把运行环境目录放在最前面。
fn child_path(dir: &Path) -> OsString {
    let base = std::env::var_os("PATH").unwrap_or_default();
    let mut parts = vec![dir.as_os_str().to_owned()];
    parts.extend(std::env::split_paths(&base).map(|p| p.into_os_string()));
    std::env::join_paths(parts).unwrap_or(base)
}

/// 会话输出在日志文件里的来源标记。
///
/// stdout 与 stderr 合流后无法再分辨属于哪台设备，多设备并发时没有来源的日志文件基本不可用。
fn log_source(alias: &str) -> String {
    format!("session {alias}")
}

/// 追加一行输出：进内存缓冲（界面取回），并落盘到当日日志文件（当日志已初始化时）。
///
/// 两处收到的是**同一份**过滤结果 —— 空行剔除与 `\r` 修剪只做一次，否则会出现
/// 「文件里有空行、界面上没有」这类两套规则。超过上限时丢掉最旧的五分之一（避免每行一次 O(n) 搬移）。
fn push_log(log: &Arc<Mutex<Vec<String>>>, source: &str, level: &str, line: String) {
    let line = line.trim_end_matches('\r').to_string();
    if line.trim().is_empty() {
        return;
    }
    logger::log(level, source, &line);
    let mut buf = log.lock().unwrap();
    if buf.len() >= MAX_LOG_LINES {
        let drop = MAX_LOG_LINES / 5;
        buf.drain(0..drop);
    }
    buf.push(line);
}

/// 逐行读取子进程输出。
///
/// scrcpy 的 info/debug 级别走 stdout、warn/error 级别走 stderr
/// （见 `doc/scrcpy/app/src/util/log.c::sc_sdl_log_print`），两股都收，合并进同一个缓冲区。
fn spawn_reader<R: Read + Send + 'static>(
    reader: R,
    log: Arc<Mutex<Vec<String>>>,
    source: String,
) {
    std::thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            match line {
                Ok(l) => push_log(&log, &source, logger::LEVEL_INFO, l),
                Err(_) => break,
            }
        }
    });
}

/// 启动子进程并接管其输出。
fn spawn_child(
    bin: &Path,
    args: &[String],
    cwd: &Path,
    source: &str,
) -> Result<(Child, Arc<Mutex<Vec<String>>>), String> {
    let mut cmd = Command::new(bin);
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // scrcpy 找 adb 的顺序是：`ADB` 环境变量 → `PATH` 里的 `adb`
        // （见 `doc/scrcpy/app/src/adb/adb.c::sc_adb_init`）。运行环境目录不在系统 PATH 上，
        // 故两条路径都指过去 —— 否则 scrcpy 会去用系统里另一个 adb，甚至根本找不到。
        .env("ADB", cwd.join(assets::ADB_BIN))
        .env("PATH", child_path(cwd));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // 不弹控制台窗口：日志已由管道接管。scrcpy.exe 是控制台应用，缺这个标志会在
        // 投屏窗口旁边多出一个黑色控制台。
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|e| format!("启动 scrcpy 失败：{e}"))?;
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    if let Some(out) = child.stdout.take() {
        spawn_reader(out, Arc::clone(&log), source.to_string());
    }
    if let Some(err) = child.stderr.take() {
        spawn_reader(err, Arc::clone(&log), source.to_string());
    }
    Ok((child, log))
}

/// 进程是否已退出；是则给出退出码。
fn try_finish(child: &Arc<Mutex<Child>>) -> Option<i32> {
    let mut c = child.lock().unwrap();
    match c.try_wait() {
        Ok(Some(status)) => Some(exit_code(status)),
        _ => None,
    }
}

/// 优雅结束进程：`taskkill` 不带 `/F`（向窗口发关闭请求，让 scrcpy 自己退出）。
#[cfg(windows)]
fn graceful_close(pid: u32) -> Result<(), String> {
    let pid = pid.to_string();
    device::run_captured(
        Path::new("taskkill.exe"),
        &["/PID", &pid],
        Duration::from_secs(5),
    )?;
    Ok(())
}

/// 见 [`graceful_close`] 的 Windows 版本（design D6）。
///
/// 非 Windows 向进程发 `SIGTERM`：scrcpy 自带信号处理，会按自身流程退出并收尾
/// （spec `connection-session`「会话的优雅结束」）。之所以用 `libc::kill` 而不是调
/// `/bin/kill`：那要依赖 util-linux（极小化容器里可能没有），还得解析它的输出与退出码；
/// 而 `libc` 只是符号绑定，不引入构建期 C 依赖。发不出信号（进程已消失等）按失败上抛，
/// 由 `stop_child` 走「优雅失败 → 强杀」的既有路径。
#[cfg(unix)]
fn graceful_close(pid: u32) -> Result<(), String> {
    let rc = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
    if rc != 0 {
        return Err(format!(
            "向进程 {pid} 发送终止信号失败：{}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

/// 结束进程：先优雅、超时兜底强杀；返回退出码。
fn stop_child(child: &Arc<Mutex<Child>>, pid: u32, timeout: Duration) -> Result<i32, String> {
    if let Some(code) = try_finish(child) {
        return Ok(code);
    }
    let _ = graceful_close(pid);
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(code) = try_finish(child) {
            return Ok(code);
        }
        if Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(STOP_TICK.min(timeout));
    }

    // 兜底：无窗口会话（--no-window）收不到关闭请求，只能强杀。
    logger::warn(
        "session",
        format!("pid={pid} 优雅结束未在限期内生效，改为强制终止"),
    );
    {
        let mut c = child.lock().unwrap();
        if let Ok(Some(status)) = c.try_wait() {
            return Ok(exit_code(status));
        }
        c.kill().map_err(|e| format!("结束会话失败：{e}"))?;
    }
    // 等它真正退出：否则槽位与「已连接」状态会被一个正在死的进程占着。
    let deadline = Instant::now() + GRACEFUL_TIMEOUT;
    loop {
        if let Some(code) = try_finish(child) {
            return Ok(code);
        }
        if Instant::now() >= deadline {
            return Ok(-1);
        }
        std::thread::sleep(STOP_TICK);
    }
}

/// 记录退出码并写一行日志（幂等：已完成的不重复记）。
fn record_exit(record: &SessionRecord, code: i32) {
    {
        let mut exit = record.exit.lock().unwrap();
        if exit.is_some() {
            return;
        }
        *exit = Some(code);
    }
    // 非零退出码按「可恢复的异常」记：排查时按日志级别就能筛出异常结束的会话。
    // 退出码本身即「正常结束 / 异常结束」的判据（退出路径不区分是谁发起的 —— 见 tasks 5.1 的实施说明）。
    let level = if code == 0 {
        logger::LEVEL_INFO
    } else {
        logger::LEVEL_WARN
    };
    record.push_log_at(level, format!("[会话结束] scrcpy 已退出，退出码 {code}"));
}

/// 回收已退出的会话（把退出码写进记录）。返回是否有记录发生变化。
///
/// 由设备状态订阅线程的周期杂务调用：进程退出既可能来自用户主动断开，也可能是异常退出，
/// 无论哪种都要及时反映到设备状态与槽位占用上。
pub fn reap_exited(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let mut changed = false;
    for record in records(&state) {
        if !record.running() {
            continue;
        }
        let status = { record.child.lock().unwrap().try_wait() };
        if let Ok(Some(status)) = status {
            record_exit(&record, exit_code(status));
            changed = true;
        }
    }
    changed
}

/// 丢弃已结束会话中最早的一批，保持记录数量有界。
fn prune_ended(map: &mut HashMap<String, SessionRecord>) {
    let mut ended: Vec<(String, u64)> = map
        .values()
        .filter(|r| !r.running())
        .map(|r| (r.device_id.clone(), r.started_at))
        .collect();
    if ended.len() <= MAX_ENDED_SESSIONS {
        return;
    }
    ended.sort_by_key(|(_, started)| *started);
    for (id, _) in ended.iter().take(ended.len() - MAX_ENDED_SESSIONS) {
        map.remove(id);
    }
}

// ── 启动会话 ───────────────────────────────────────────────────

/// 离线设备发起会话前的动作（纯函数，design D1）。
#[derive(Debug, PartialEq, Eq)]
enum LinkAction {
    /// 无线设备：先建立链路，成败由随后的重探测判定。
    Establish,
    /// USB 设备：不做连接尝试 —— 它的在线与否由物理接入决定。
    Refuse,
}

fn link_action(kind: &str) -> LinkAction {
    if kind == config::KIND_WIFI {
        LinkAction::Establish
    } else {
        LinkAction::Refuse
    }
}

/// 状态显示「在线」时，启动会话前是否仍要先建立链路（纯函数）。
///
/// 缓存会滞后于 adb 的实际列表：替换运行环境后服务端是新实例、无线链路已丢，
/// 应用外的 `adb disconnect` 也不会立刻反映。此时若信任缓存直接启动，scrcpy 会以
/// 「Could not find any ADB device」退出（用户看到的是「第一次失败、第二次成功」）。
///
/// 只看两件事实：设备是无线、且不在实际列表里。USB 设备不在此列 —— 它的在线与否由
/// 物理接入决定，既有规则也禁止对它做连接尝试。`present_in_live_list` 在复核不可用时
/// 按「视为在场」传入，因此这条判断不会因复核故障而阻断连接。
fn needs_link_before_start(kind: &str, present_in_live_list: bool) -> bool {
    kind == config::KIND_WIFI && !present_in_live_list
}

/// 建立链路之后按重探测结果判定成败（纯函数，design D2）。
///
/// 成功口径是**重探测后的设备状态**，不是 `adb connect` 的退出码：那条命令连不上时也可能
/// 以 0 退出（原因写在输出里），反过来输出写着 connected 也不代表设备可用（可能仍 offline /
/// unauthorized）。判定与界面同源 —— 成不成等于用户看到的那个状态。
///
/// 失败原因取 adb 的原始输出原文（不做加工、不截断）：用户要能区分端口未开、IP 变更与
/// 设备端异常，「连接失败」四个字分辨不了。
fn link_outcome(status_after: Option<&str>, output: &str) -> Result<(), String> {
    if status_after == Some(device::STATUS_IDLE) {
        return Ok(());
    }
    let reason = output.trim();
    Err(format!(
        "无法建立无线链路：{}",
        if reason.is_empty() {
            "设备仍不可达"
        } else {
            reason
        }
    ))
}

/// 为离线的无线设备建立链路，并按重探测结果判定成败（design D1 / D2）。
fn ensure_wireless_link(app: &AppHandle, state: &AppState, device: &Device) -> Result<(), String> {
    let dir = scrcpy::scrcpy_dir(app)?;
    let connection_id = device.connection_id();
    // 执行层面的失败（缺 adb / 超时 / 非零退出）本身就是这次建立失败的原因。
    let output = device::connect_link(&dir, &connection_id).map_err(|e| {
        logger::warn("device", format!("无线链路建立失败（{}）：{e}", device.alias));
        format!("无法建立无线链路：{e}")
    })?;
    // 不等订阅的下一帧：界面与顶部计数在本次操作内就该反映出来。
    device::refresh_probe(app);
    device::publish_status(app);
    let after = device::status_of_device(state, &device.id);
    match link_outcome(after.as_deref(), &output) {
        Ok(()) => {
            logger::info(
                "device",
                format!("无线链路已建立（{}）：{}", device.alias, output.trim()),
            );
            Ok(())
        }
        Err(e) => {
            logger::warn("device", format!("无线链路建立失败（{}）：{e}", device.alias));
            Err(e)
        }
    }
}

/// 启动一个会话并登记。返回是否发生槽位复用（降级）。
fn start_session(app: &AppHandle, device: &Device, args: Vec<String>) -> Result<bool, String> {
    let state = app.state::<AppState>();
    let dir = scrcpy::scrcpy_dir(app)?;
    let missing = assets::missing_bins(&dir);
    if !missing.is_empty() {
        return Err(format!(
            "运行环境未就绪：缺少 {}。可在「更多」页获取，或手动放置后点「重新检测」。",
            missing.join("、")
        ));
    }
    let connection_id = device.connection_id();
    if connection_id.is_empty() {
        return Err("该设备的连接标识尚未确定（USB serial 待检测），无法发起会话".to_string());
    }
    if active_record(&state, &device.id).is_some() {
        return Err("该设备已有会话在运行".to_string());
    }
    // 状态判定以**后端**为准（tasks 9.2），不依赖前端的筛选。
    match device::status_of_device(&state, &device.id).as_deref() {
        // 缓存显示在线不等于 adb 里真的在：启动前复核一次（一次本地查询，不创建子进程）。
        // 复核失败时按「视为在场」继续 —— 减少失败的手段不能变成新的失败来源。
        Some(device::STATUS_IDLE) => {
            let present = match device::live_devices(app) {
                Ok(live) => live.contains(&connection_id),
                Err(e) => {
                    logger::warn(
                        "device",
                        format!("连接前复核「{}」失败：{e}（沿用界面状态继续）", device.alias),
                    );
                    true
                }
            };
            if needs_link_before_start(&device.kind, present) {
                ensure_wireless_link(app, &state, device)?;
            }
        }
        Some(device::STATUS_CONNECTED) => return Err("该设备已有会话在运行".to_string()),
        // 无线设备可能已被应用之外的操作断开（如用户在自己的终端里 `adb disconnect`）：
        // 先把链路建起来，再按重探测结果决定是否继续（design D1 / D2）。
        Some(device::STATUS_OFFLINE) => match link_action(&device.kind) {
            LinkAction::Establish => ensure_wireless_link(app, &state, device)?,
            LinkAction::Refuse => {
                return Err("设备离线或连接标识未知，暂无法发起会话（请确认设备已连接）".to_string())
            }
        },
        Some(_) => {
            return Err("设备离线或连接标识未知，暂无法发起会话（请确认设备已连接）".to_string())
        }
        None => return Err("设备不存在".to_string()),
    }

    // 只有已经在摆窗口时才知道「真实窗口有多大」才有意义：第一个窗口落在
    // (margin, margin)，与尺寸无关，因此不必为它多跑一次 adb 查询。
    //
    // 显示协议不允许应用定位窗口（如 Wayland 会话）时，自动排列就是假的 —— scrcpy 会把
    // 位置参数交给一个会无视它们的合成器。此时明确降级为「不参与排列」并让界面说明原因
    // （design D5），MUST NOT 排出一个叠在一起的位置，也不必再为它跑显示尺寸查询。
    let occupied = occupied_slots(&state);
    let plan = if can_position_now() {
        let display = if occupied.is_empty() {
            None
        } else {
            query_display_size(&dir.join(assets::ADB_BIN), &connection_id)
        };
        plan_arrangement(
            &occupied,
            &work_area(app),
            &args,
            window_frame_insets(),
            display,
        )
    } else {
        Arrangement::opt_out(ArrangeSkipReason::Platform)
    };

    let bin = dir.join(scrcpy::SCRCPY_BIN);
    let full = session_argv(&connection_id, &args, &plan);
    // 会话输出的来源标记在 spawn 时就要定下来：两个读取线程与后面登记的记录必须用同一个。
    let source = log_source(&device.alias);
    let (child, log) = spawn_child(&bin, &full, &dir, &source)?;
    let pid = child.id();

    let record = SessionRecord {
        device_id: device.id.clone(),
        alias: device.alias.clone(),
        connection_id: connection_id.clone(),
        pid,
        started_at: now_ms(),
        command: command_line(&full),
        arranged: plan.arranged,
        arrange_skip: plan.opt_out,
        slot: plan.slot,
        window_x: plan.x,
        window_y: plan.y,
        slot_reused: plan.reused,
        log,
        child: Arc::new(Mutex::new(child)),
        exit: Arc::new(Mutex::new(None)),
    };
    // 启动行含实际下发的**完整命令**：自动排列写入的 `--window-x` / `--window-y`（以及用户自己
    // 指定的那两个 flag）都在命令里，故窗口位置不必另写一段 —— 命令本身就是可复现的全部现场。
    record.push_log(format!("[会话启动] pid={pid} 命令：{}", record.command));
    if !plan.arranged {
        let why = match plan.opt_out {
            Some(ArrangeSkipReason::Platform) => {
                "当前显示协议不允许应用指定窗口位置，窗口位置由系统决定"
            }
            Some(ArrangeSkipReason::User) | None => "该设备已指定 --window-x / --window-y",
        };
        record.push_log(format!("[窗口排列] {why}，未参与自动排列"));
    } else if plan.reused {
        // 空间不足导致的降级按异常记：窗口可能互相遮挡，是排查「窗口怎么叠在一起」的第一手线索。
        record.push_log_at(
            logger::LEVEL_WARN,
            format!("[窗口排列] {}", degraded_warning()),
        );
    }

    {
        let mut map = state.sessions.lock().unwrap();
        prune_ended(&mut map);
        map.insert(record.device_id.clone(), record);
    }
    Ok(plan.reused)
}

fn outcome_ok(device: &Device) -> ConnectOutcome {
    ConnectOutcome {
        device_id: device.id.clone(),
        alias: device.alias.clone(),
        ok: true,
        error: None,
    }
}

fn outcome_fail(device_id: &str, alias: &str, error: String) -> ConnectOutcome {
    ConnectOutcome {
        device_id: device_id.to_string(),
        alias: alias.to_string(),
        ok: false,
        error: Some(error),
    }
}

// ── 结束会话 ───────────────────────────────────────────────────

/// 结束一台设备的会话（若在运行）；返回是否有会话被结束。
fn stop_session(app: &AppHandle, device_id: &str) -> Result<bool, String> {
    let state = app.state::<AppState>();
    let record = { state.sessions.lock().unwrap().get(device_id).cloned() };
    let Some(record) = record else { return Ok(false) };
    if !record.running() {
        return Ok(false);
    }
    match stop_child(&record.child, record.pid, GRACEFUL_TIMEOUT) {
        Ok(code) => {
            record_exit(&record, code);
            Ok(true)
        }
        Err(e) => Err(e),
    }
}

/// 结束会话并丢弃其记录（设备被删除时用：记录留着没有意义，还会一直占着内存）。
pub fn stop_and_forget(app: &AppHandle, device_id: &str) -> Result<(), String> {
    let stopped = stop_session(app, device_id)?;
    let state = app.state::<AppState>();
    let changed = { state.sessions.lock().unwrap().remove(device_id).is_some() };
    if stopped || changed {
        publish_sessions(app, &session_infos(&state));
        device::publish_status(app);
    }
    Ok(())
}

/// 应用退出时结束全部会话：会话由本应用持有，否则会留下无人管理的投屏窗口。
pub fn shutdown_all(app: &AppHandle) {
    let state = app.state::<AppState>();
    for record in records(&state) {
        if !record.running() {
            continue;
        }
        // 退出路径上不再等待太久：优雅退出优先，但绝不让关闭流程被卡住。
        let _ = stop_child(&record.child, record.pid, Duration::from_millis(500));
    }
}

// ── Tauri 命令 ─────────────────────────────────────────────────

/// 连接单台设备（tasks 9.1）。
///
/// 参数由前端拼装并传入（字段模型归前端所有），本命令补 `-s <连接标识>` 与自动排列的位置参数。
#[tauri::command]
pub async fn connect_device(
    app: AppHandle,
    request: ConnectRequest,
) -> Result<ConnectResult, String> {
    run_blocking(move || connect_blocking(&app, request)).await
}

fn connect_blocking(app: &AppHandle, request: ConnectRequest) -> Result<ConnectResult, String> {
    let state = app.state::<AppState>();
    let device = find_device(&state, &request.device_id)?;
    let degraded = start_session(app, &device, request.args)?;
    let sessions = session_infos(&state);
    publish_sessions(app, &sessions);
    // 会话一起，设备状态就要立刻变成「已连接」（不等订阅的下一帧）。
    device::publish_status(app);
    Ok(ConnectResult {
        outcomes: vec![outcome_ok(&device)],
        sessions,
        degraded,
        warning: degraded.then(degraded_warning),
    })
}

/// 全部连接（tasks 9.2）：逐台尝试，**离线设备由后端直接跳过**（不会去拉起进程）。
#[tauri::command]
pub async fn connect_all(
    app: AppHandle,
    requests: Vec<ConnectRequest>,
) -> Result<ConnectResult, String> {
    run_blocking(move || connect_all_blocking(&app, requests)).await
}

fn connect_all_blocking(
    app: &AppHandle,
    requests: Vec<ConnectRequest>,
) -> Result<ConnectResult, String> {
    let state = app.state::<AppState>();
    let mut outcomes = Vec::new();
    let mut degraded = false;
    for request in requests {
        let device = match find_device(&state, &request.device_id) {
            Ok(d) => d,
            Err(e) => {
                outcomes.push(outcome_fail(&request.device_id, "", e));
                continue;
            }
        };
        match start_session(app, &device, request.args) {
            Ok(reused) => {
                degraded |= reused;
                outcomes.push(outcome_ok(&device));
            }
            Err(e) => outcomes.push(outcome_fail(&device.id, &device.alias, e)),
        }
    }
    let sessions = session_infos(&state);
    publish_sessions(app, &sessions);
    device::publish_status(app);
    Ok(ConnectResult {
        outcomes,
        sessions,
        degraded,
        warning: degraded.then(degraded_warning),
    })
}

/// 断开单台设备（tasks 9.1）：结束会话后该设备回到空闲 —— 界面上的「在线」（tasks 9.7 槽位随之释放）。
#[tauri::command]
pub async fn disconnect_device(
    app: AppHandle,
    device_id: String,
) -> Result<Vec<SessionInfo>, String> {
    run_blocking(move || disconnect_blocking(&app, &device_id)).await
}

fn disconnect_blocking(app: &AppHandle, device_id: &str) -> Result<Vec<SessionInfo>, String> {
    let state = app.state::<AppState>();
    {
        let has_record = state.sessions.lock().unwrap().contains_key(device_id);
        if !has_record {
            return Err("该设备当前没有会话".to_string());
        }
    }
    stop_session(app, device_id)?;
    let sessions = session_infos(&state);
    publish_sessions(app, &sessions);
    device::publish_status(app);
    Ok(sessions)
}

/// 全部断开（tasks 9.2）：批量结束全部在运行的会话。
#[tauri::command]
pub async fn disconnect_all(app: AppHandle) -> Result<Vec<SessionInfo>, String> {
    run_blocking(move || disconnect_all_blocking(&app)).await
}

fn disconnect_all_blocking(app: &AppHandle) -> Result<Vec<SessionInfo>, String> {
    let state = app.state::<AppState>();
    for record in records(&state) {
        if !record.running() {
            continue;
        }
        match stop_child(&record.child, record.pid, GRACEFUL_TIMEOUT) {
            Ok(code) => record_exit(&record, code),
            // 单个会话结束失败不影响其它会话（tasks 9.3：会话之间互不干扰）。
            Err(e) => record.push_log(format!("[会话结束] 结束失败：{e}")),
        }
    }
    let sessions = session_infos(&state);
    publish_sessions(app, &sessions);
    device::publish_status(app);
    Ok(sessions)
}

/// 会话列表（含已结束的会话，供「更多」页的日志与诊断查看）。
#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> Vec<SessionInfo> {
    session_infos(&state)
}

/// 某个会话的完整输出（tasks 9.4）。
#[tauri::command]
pub fn get_session_log(state: State<'_, AppState>, device_id: String) -> Result<String, String> {
    let record = { state.sessions.lock().unwrap().get(&device_id).cloned() }
        .ok_or_else(|| "该设备没有会话记录".to_string())?;
    let lines = record.log.lock().unwrap().clone();
    Ok(lines.join("\n"))
}

/// 清空某个会话已保留的输出（「日志」页的「清空」）。
#[tauri::command]
pub fn clear_session_log(state: State<'_, AppState>, device_id: String) -> Result<(), String> {
    clear_log(&state.sessions, &device_id)
}

#[cfg(test)]
mod tests {
    //! 可验证部分（tasks 9.5–9.8）：
    //! - 槽位分配与几何计算是纯函数，直接用已知尺寸验证；
    //! - 「显式指定位置则不参与排列」的判定取自将要下发的参数本身；
    //! - 进程启动 / 输出捕获 / 结束用系统自带命令验证（不依赖真实 adb 与设备）。

    use super::*;

    fn area() -> Area {
        Area {
            x: 0,
            y: 0,
            width: 1920,
            height: 1040,
        }
    }

    fn set(items: &[usize]) -> BTreeSet<usize> {
        items.iter().copied().collect()
    }

    // ── 9.5：每行可容纳数量 ────────────────────────────────────

    #[test]
    fn columns_follow_the_layout_formula() {
        // design D6 的公式：floor((1920 - 16 + 8) / (480 + 8)) = floor(1912 / 488) = 3
        assert_eq!(columns(1920, 480, TILE_MARGIN, TILE_GAP), 3);
        // 1280 宽：floor((1280 - 16 + 8) / 488) = floor(1272 / 488) = 2
        assert_eq!(columns(1280, 480, TILE_MARGIN, TILE_GAP), 2);
        // 窗口比工作区还宽时至少 1 列（不能算出 0 列让排列无解）
        assert_eq!(columns(400, 480, TILE_MARGIN, TILE_GAP), 1);
        // 高度方向同源：floor((1040 - 16 + 8) / (900 + 8)) = 1
        assert_eq!(rows(1040, DEFAULT_WINDOW_H, TILE_MARGIN, TILE_GAP), 1);
        assert_eq!(capacity(3, 1), 3);
    }

    // ── 9.6：行优先排列、互不重叠 ──────────────────────────────

    #[test]
    fn slots_are_placed_row_major() {
        let a = area();
        let cols = 3;
        let p0 = slot_position(0, cols, &a, 480, 900, TILE_MARGIN, TILE_GAP);
        let p1 = slot_position(1, cols, &a, 480, 900, TILE_MARGIN, TILE_GAP);
        let p2 = slot_position(2, cols, &a, 480, 900, TILE_MARGIN, TILE_GAP);
        let p3 = slot_position(3, cols, &a, 480, 900, TILE_MARGIN, TILE_GAP);

        // 先水平：同一行 y 相同、x 依次递增 488。
        assert_eq!(p0, (TILE_MARGIN, TILE_MARGIN));
        assert_eq!(p1, (TILE_MARGIN + 488, TILE_MARGIN));
        assert_eq!(p2, (TILE_MARGIN + 976, TILE_MARGIN));
        // 排满换行：回到最左侧，y 下移一行。
        assert_eq!(p3, (TILE_MARGIN, TILE_MARGIN + 908));

        // 同一行内两两不重叠（间距 = gap）。
        assert!(p0.0 + 480 + TILE_GAP <= p1.0);
        assert!(p1.0 + 480 + TILE_GAP <= p2.0);
        // 换行后也不与上一行重叠。
        assert!(p0.1 + 900 + TILE_GAP <= p3.1);
    }

    // ── 9.7 / 9.8：槽位释放复用与空间不足降级 ──────────────────

    #[test]
    fn free_slot_is_reused_before_growing() {
        // 1 号槽位释放后，下一个会话应回到 1 号（而不是继续往后排）。
        assert_eq!(allocate_slot(&set(&[0, 2]), 6), (1, false));
        assert_eq!(allocate_slot(&set(&[]), 6), (0, false));
    }

    #[test]
    fn shortage_reuses_slots_and_reports_degradation() {
        // 容量已满：复用最小已占用槽位并报降级，绝不返回失败（tasks 9.8）。
        assert_eq!(allocate_slot(&set(&[0, 1]), 2), (0, true));
        // 容量为 0 的极端输入同样退化为 1 个槽位。
        assert_eq!(allocate_slot(&set(&[0]), 0), (0, true));
    }

    // ── 9.9：用户显式指定窗口位置时让位 ────────────────────────

    #[test]
    fn explicit_window_position_opts_out_of_arrangement() {
        let a = area();
        let none = set(&[]);
        let bare = FrameInsets::default();

        // 未指定 → 参与排列。
        let p = plan_arrangement(&none, &a, &["--window-width=640".to_string()], bare, None);
        assert!(p.arranged);
        assert_eq!(p.slot, Some(0));

        // `--window-x` / `--window-y`（含自定义参数串里的写法）→ 不参与排列。
        for args in [
            vec!["--window-x=100".to_string()],
            vec!["--window-y".to_string(), "50".to_string()],
            vec!["--fullscreen --window-x=300".to_string()],
        ] {
            let plan = plan_arrangement(&none, &a, &args, bare, None);
            assert!(!plan.arranged, "args = {args:?}");
            assert_eq!(plan.opt_out, Some(ArrangeSkipReason::User), "args = {args:?}");
            assert_eq!(plan.slot, None);
            assert_eq!(plan.x, None);
            assert_eq!(plan.y, None);
        }
    }

    #[test]
    fn wayland_session_opts_out_of_arrangement() {
        // tasks 5.1：显示协议不允许应用定位窗口时，排列整段让位 —— 不派发 `--window-x` /
        // `--window-y`（scrcpy 会把它们交给一个会无视参数的合成器），并由界面说明原因。
        let plan = Arrangement::opt_out(ArrangeSkipReason::Platform);
        let argv = session_argv("mi10:5555", &["--max-fps=30".to_string()], &plan);

        assert!(!plan.arranged);
        assert_eq!(plan.opt_out, Some(ArrangeSkipReason::Platform));
        assert_eq!(plan.x, None);
        assert_eq!(plan.y, None);
        // 命令里不得出现任何位置参数：发了也是白发（Wayland 下被合成器无视）。
        assert!(!argv.iter().any(|a| a.starts_with("--window-x")));
        assert!(!argv.iter().any(|a| a.starts_with("--window-y")));
    }

    #[test]
    fn position_capability_follows_the_session_env() {
        // tasks 5.1：判据用环境变量，且做成纯函数 —— 两种输入在任一平台上都可验证。
        // X11 会话：可定位。
        assert!(can_position_windows(Some("x11"), None));
        // 未设任何变量：按可定位处理（Windows 与未知的 Wayland 变体都不该误伤）。
        assert!(can_position_windows(None, None));
        // Wayland 会话的两种信号：会话类型，或直接暴露的 Wayland 显示连接。
        assert!(!can_position_windows(Some("wayland"), None));
        assert!(!can_position_windows(None, Some("wayland-0")));
        // 大小写与空白都容忍（发行版里两种写法都有）。
        assert!(!can_position_windows(Some("Wayland"), None));
        assert!(!can_position_windows(Some(" wayland "), Some("  ")));
        // 用户显式指定的位置不受此影响：两种原因的优先级在 plan_arrangement 里已定（用户优先）。
        assert_eq!(
            Arrangement::opt_out(ArrangeSkipReason::Platform).opt_out,
            Some(ArrangeSkipReason::Platform)
        );
    }

    /// 窗口装饰必须计入排列：否则相邻两行会重叠、首行标题栏会越出工作区上沿。
    ///
    /// 数值取自实测（内容区 400 高的窗口整体高 439：标题栏+上边框 31、下边框 8、左右边框各 8）。
    #[test]
    fn frame_insets_keep_rows_apart_and_window_on_screen() {
        let a = area();
        let none = set(&[]);
        let insets = FrameInsets {
            left: 8,
            top: 31,
            width: 16,
            height: 39,
        };
        let args = vec![
            "--window-width=480".to_string(),
            "--window-height=400".to_string(),
        ];

        // 步长按「整体尺寸」算：高 439 + 间距 8 = 447 → 1920x1040 的工作区仍是 3 列 2 行。
        let cols = columns(a.width, 480 + insets.width, TILE_MARGIN, TILE_GAP);
        let rows_n = rows(a.height, 400 + insets.height, TILE_MARGIN, TILE_GAP);
        assert_eq!((cols, rows_n), (3, 2));

        let first = plan_arrangement(&none, &a, &args, insets, None);
        assert_eq!(first.slot, Some(0));
        // 返回的是内容区左上角：整体左上角 (8, 8) + 装饰内缩 (8, 31)。
        assert_eq!((first.x, first.y), (Some(16), Some(39)));
        // 整体左上角仍在工作区内（标题栏不会被推到屏幕外）。
        assert!(first.y.unwrap() - insets.top >= a.y);

        let fourth = plan_arrangement(&set(&[0, 1, 2]), &a, &args, insets, None);
        assert_eq!(fourth.slot, Some(3));
        let frame_top0 = first.y.unwrap() - insets.top;
        let frame_top3 = fourth.y.unwrap() - insets.top;
        assert_eq!(frame_top3 - frame_top0, 400 + insets.height + TILE_GAP);
        // 第 0 行窗口的底边（整体坐标）与第 1 行窗口的顶边之间恰好留下 gap —— 不重叠。
        let frame_bottom0 = frame_top0 + 400 + insets.height;
        assert_eq!(frame_top3 - frame_bottom0, TILE_GAP);
        assert_eq!(fourth.x, first.x, "换行后回到最左列");
    }

    #[test]
    fn user_window_size_feeds_the_layout_math() {
        let a = area();
        // 用 900 宽的窗口：floor((1920 - 16 + 8) / (900 + 8)) = 2 列
        let tokens = vec!["--window-width=900".to_string()];
        assert_eq!(int_flag(&flat_tokens(&tokens), "--window-width"), Some(900));
        let cols = columns(a.width, 900, TILE_MARGIN, TILE_GAP);
        assert_eq!(cols, 2);
        // 1 号槽位落在第二列
        assert_eq!(
            slot_position(1, cols, &a, 900, DEFAULT_WINDOW_H, TILE_MARGIN, TILE_GAP).0,
            TILE_MARGIN + 908
        );
    }

    // ── 窗口尺寸估算（真实设备尺寸 → 实际会开出的窗口内容区）──────

    /// 实测取自本机 Mi 10（Android 13）的 `dumpsys display` 片段。
    const DUMPSYS_PORTRAIT: &str = "    mBaseDisplayInfo=DisplayInfo{\"内置屏幕\", displayId 0\", displayGroupId 0, FLAG_SECURE, real 1080 x 2340, largest app 1080 x 2340, smallest app 1080 x 2340, mode 1, defaultMode 1, modes [{id=1, width=1080, height=2340}], rotation 0, state OFF, type INTERNAL, app 1080 x 2340, density 440}\n";

    #[test]
    fn parses_display_size_and_applies_rotation() {
        assert_eq!(parse_display_size(DUMPSYS_PORTRAIT), Some((1080, 2340)));
        // rotation 1（横屏）：宽高互换。
        let landscape = DUMPSYS_PORTRAIT.replace("rotation 0", "rotation 1");
        assert_eq!(parse_display_size(&landscape), Some((2340, 1080)));
        // 解析不出内容时返回 None（调用方退回按请求值估算）。
        assert_eq!(parse_display_size("no useful output"), None);
        assert_eq!(parse_display_size(""), None);
    }

    /// 实测结论：请求 480x400 的手机竖屏窗口，scrcpy 实际只开出约 185x400（宽高比锁）。
    #[test]
    fn fits_requested_box_to_content_aspect() {
        let content = (1080, 2340);
        let bounds = (1904, 1024);

        // 两个都给：按内容比例收窄到不超出请求框。
        assert_eq!(fit_content(content, 480, 400, bounds), (185, 400));
        // 只给高：另一个按比例推导。
        assert_eq!(fit_content(content, 0, 400, bounds), (185, 400));
        // 只给宽：另一个按比例推导。
        assert_eq!(fit_content(content, 480, 0, bounds), (480, 1040));
        // 都不给：内容尺寸缩进边界内（不放大），等效 scrcpy 的 get_optimal_size(content, content)。
        assert_eq!(fit_content(content, 0, 0, bounds), (473, 1024));
        // 内容本来就落在边界内：保持内容尺寸，不放大。
        assert_eq!(fit_content((540, 1000), 0, 0, bounds), (540, 1000));
    }

    #[test]
    fn content_size_follows_scrcpy_flags() {
        let display = Some((1080, 2340));
        let t = |args: &[&str]| flat_tokens(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>());

        // 普通镜像：内容尺寸就是显示尺寸。
        assert_eq!(content_size(&t(&[]), display), Some((1080, 2340)));
        // 虚拟显示器直接给定内容尺寸。
        assert_eq!(
            content_size(&t(&["--new-display=1920x1080/420"]), display),
            Some((1920, 1080))
        );
        // `--crop` 直接给定内容尺寸。
        assert_eq!(
            content_size(&t(&["--crop=1224:1440:0:0"]), display),
            Some((1224, 1440))
        );
        // `--angle=90` 转置。
        assert_eq!(content_size(&t(&["--angle=90"]), display), Some((2340, 1080)));
        // `--max-size`（前端用短选项 `-m`）把长边限到 N。
        assert_eq!(content_size(&t(&["-m", "720"]), display), Some((332, 720)));
        // 与设备分辨率无关的模式：不猜。
        assert_eq!(content_size(&t(&["--video-source=camera"]), display), None);
        assert_eq!(content_size(&t(&["--flex-display"]), display), None);
        assert_eq!(content_size(&t(&["--display-id=2"]), display), None);
        // 拿不到显示尺寸（查询失败）同样是 None。
        assert_eq!(content_size(&t(&[]), None), None);
    }

    /// 改造的核心：排列步长要按**真实窗口宽度**（185 + 装饰 16 = 201）算，而不是请求的 480。
    #[test]
    fn arrangement_uses_real_window_width_as_step() {
        let a = area();
        let insets = FrameInsets {
            left: 8,
            top: 31,
            width: 16,
            height: 39,
        };
        let args = vec![
            "--window-width=480".to_string(),
            "--window-height=400".to_string(),
        ];

        // 已知设备显示尺寸：按真实窗口（201 宽 + 8 间距 = 209 步长）排，一行可放 9 个。
        let known = plan_arrangement(&set(&[]), &a, &args, insets, Some((1080, 2340)));
        assert_eq!(known.slot, Some(0));
        // 槽位 0 的内容区左上角 = 工作区 (0,0) + 留白 8 + 装饰内缩 (8, 31)。
        assert_eq!((known.x, known.y), (Some(16), Some(39)));
        let second = plan_arrangement(&set(&[0]), &a, &args, insets, Some((1080, 2340)));
        assert_eq!(second.slot, Some(1));
        // 步长 = 真实窗口宽 201 + 间距 8 = 209（改造前是 480 + 16 + 8 = 504）。
        assert_eq!(second.x, Some(16 + 201 + TILE_GAP));
        let cols = columns(a.width, 201, TILE_MARGIN, TILE_GAP);
        assert_eq!(cols, 9);
        // 一行 9 个 → 第 10 个才需要复用槽位（改造前按 480 算只有 3 列，第 4 个就被判「空间不足」）。
        let ninth = plan_arrangement(&set(&[0, 1, 2, 3, 4, 5, 6, 7]), &a, &args, insets, Some((1080, 2340)));
        assert_eq!(ninth.slot, Some(8));
        assert!(!ninth.reused);

        // 取不到显示尺寸：退回请求值（与改造前一致，不会因查询失败而变差）。
        let unknown = plan_arrangement(&set(&[0]), &a, &args, insets, None);
        assert_eq!(unknown.x, Some(16 + 480 + insets.width + TILE_GAP));
    }

    // ── 命令行与日志 ───────────────────────────────────────────

    #[test]
    fn command_line_matches_preview_shape() {
        let argv = vec![
            "-s".to_string(),
            "192.168.1.5:5555".to_string(),
            "--max-fps=30".to_string(),
            "--window-x=8".to_string(),
        ];
        let line = command_line(&argv);

        assert_eq!(line, "scrcpy -s 192.168.1.5:5555 --max-fps=30 --window-x=8");
        // `-s` 只能出现一次：显示串由 argv 直接拼出，不得再单独补一遍（曾出现 `-s X -s X`）。
        assert_eq!(line.matches("-s ").count(), 1);
    }

    #[test]
    fn session_argv_has_one_serial_and_appends_arranged_position() {
        let bare = FrameInsets::default();
        let plan = plan_arrangement(&set(&[]), &area(), &["--max-fps=30".to_string()], bare, None);
        let argv = session_argv("mi10:5555", &["--max-fps=30".to_string()], &plan);

        assert_eq!(
            argv,
            vec![
                "-s",
                "mi10:5555",
                "--max-fps=30",
                "--window-x=8",
                "--window-y=8"
            ]
        );
        // 实际 argv 与界面显示的命令行同源：都只带一个 `-s`。
        assert_eq!(command_line(&argv).matches("-s ").count(), 1);

        // 用户显式指定位置时不追加自动排列参数（让位）。
        let opted = plan_arrangement(
            &set(&[]),
            &area(),
            &["--window-x=100".to_string()],
            bare,
            None,
        );
        assert_eq!(
            session_argv("mi10:5555", &["--window-x=100".to_string()], &opted),
            vec!["-s", "mi10:5555", "--window-x=100"]
        );
    }

    #[test]
    fn log_buffer_is_capped() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        for i in 0..(MAX_LOG_LINES + 100) {
            push_log(&log, "会话 测试", logger::LEVEL_INFO, format!("line {i}"));
        }
        let buf = log.lock().unwrap();
        assert!(buf.len() <= MAX_LOG_LINES);
        // 保留的是最后一批（最早的被丢弃）
        assert!(buf.last().unwrap().ends_with(&format!("{}", MAX_LOG_LINES + 99)));
    }

    /// 造一条真实记录：`child` 必须是有效句柄，故启一个立刻退出的 `cmd /C exit`
    /// （它只为填住 `Child` 字段；运行与否一律由 `exit` 决定，与本用例无关）。
    #[cfg(windows)]
    fn record_with_log(device_id: &str, lines: &[&str]) -> SessionRecord {
        let child = Command::new("cmd")
            .args(["/C", "exit"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("无法启动 cmd.exe");
        let pid = child.id();
        SessionRecord {
            device_id: device_id.to_string(),
            alias: device_id.to_string(),
            connection_id: format!("{device_id}:5555"),
            pid,
            started_at: now_ms(),
            command: "scrcpy -s x".to_string(),
            arranged: true,
            arrange_skip: None,
            slot: Some(0),
            window_x: Some(8),
            window_y: Some(8),
            slot_reused: false,
            log: Arc::new(Mutex::new(lines.iter().map(|l| (*l).to_string()).collect())),
            child: Arc::new(Mutex::new(child)),
            exit: Arc::new(Mutex::new(None)),
        }
    }

    #[cfg(windows)]
    #[test]
    fn clear_log_empties_only_the_target_session() {
        let running = record_with_log("dev-run", &["第一行", "第二行"]);
        let ended = {
            let record = record_with_log("dev-end", &["别的会话那行"]);
            // record_exit 既写退出码也追加一行「会话结束」—— 正合真实场景。
            record_exit(&record, 3);
            record
        };
        let mut map: HashMap<String, SessionRecord> = HashMap::new();
        map.insert("dev-run".to_string(), running);
        map.insert("dev-end".to_string(), ended);
        let sessions = Mutex::new(map);

        let lines = |id: &str| {
            sessions
                .lock()
                .unwrap()
                .get(id)
                .cloned()
                .unwrap()
                .info()
                .log_lines
        };
        assert_eq!(lines("dev-run"), 2);
        assert_eq!(lines("dev-end"), 2);

        // 运行中的会话：缓冲清空，但记录仍在、仍算运行中（设备状态不受牵连）。
        clear_log(&sessions, "dev-run").unwrap();
        let after = sessions.lock().unwrap().get("dev-run").cloned().unwrap();
        assert!(after.log.lock().unwrap().is_empty());
        assert!(after.running());
        // 清空不破坏后续追加：旧行不再回来，新行照常进缓冲。
        after.push_log("清空后的新行");
        assert_eq!(after.info().log_lines, 1);

        // 只清这一条：另一条（已结束）的输出仍完整。
        assert_eq!(lines("dev-end"), 2);

        // 已结束的会话同样可清空，且退出码与实际命令一个字都没丢。
        clear_log(&sessions, "dev-end").unwrap();
        let info = sessions.lock().unwrap().get("dev-end").cloned().unwrap().info();
        assert_eq!(info.log_lines, 0);
        assert_eq!(info.exit_code, Some(3));
        assert!(!info.running);
        assert_eq!(info.command, "scrcpy -s x");

        // 没有记录的设备：报与 `get_session_log` 同措辞的错误，且不动任何缓冲。
        assert_eq!(
            clear_log(&sessions, "dev-none").unwrap_err(),
            "该设备没有会话记录"
        );
        assert_eq!(sessions.lock().unwrap().len(), 2);
    }

    // ── 9.3：真实进程的启动 / 输出 / 结束 ──────────────────────

    #[cfg(windows)]
    #[test]
    fn captures_output_of_a_real_process() {
        let cwd = std::env::temp_dir();
        let args = vec![
            "/C".to_string(),
            "echo".to_string(),
            "HELLO-FROM-SESSION".to_string(),
        ];
        let (child, log) = spawn_child(Path::new("cmd.exe"), &args, &cwd, "会话 测试").unwrap();
        let record = SessionRecord {
            device_id: "dev-1".to_string(),
            alias: "测试".to_string(),
            connection_id: "S1".to_string(),
            pid: child.id(),
            started_at: now_ms(),
            command: "cmd.exe".to_string(),
            arranged: true,
            arrange_skip: None,
            slot: Some(0),
            window_x: Some(8),
            window_y: Some(8),
            slot_reused: false,
            log: Arc::clone(&log),
            child: Arc::new(Mutex::new(child)),
            exit: Arc::new(Mutex::new(None)),
        };

        // 等输出被读进来（读取在独立线程里做，需要等它跑完）。
        let deadline = Instant::now() + Duration::from_secs(10);
        let ok = loop {
            if log.lock().unwrap().iter().any(|l| l.contains("HELLO-FROM-SESSION")) {
                break true;
            }
            if Instant::now() >= deadline {
                break false;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(ok, "未捕获到子进程输出：{:?}", log.lock().unwrap());

        // 进程结束后被回收，退出码写入记录（会话表据此把设备状态改回空闲，即界面上的「在线」）。
        let code = stop_child(&record.child, record.pid, Duration::from_millis(200)).unwrap();
        assert_eq!(code, 0);
        record_exit(&record, code);
        assert!(!record.running());
        assert_eq!(record.info().exit_code, Some(0));
    }

    #[cfg(windows)]
    #[test]
    fn stopping_a_session_terminates_a_long_running_process() {
        // 模拟一个长时间运行的会话（ping 30 次 ≈ 30 秒）：必须被结束，且不能在
        // 优雅等待超时后仍留着进程。
        let cwd = std::env::temp_dir();
        let args = vec![
            "-n".to_string(),
            "30".to_string(),
            "127.0.0.1".to_string(),
        ];
        let (child, _log) = spawn_child(Path::new("ping.exe"), &args, &cwd, "会话 测试").unwrap();
        let child = Arc::new(Mutex::new(child));
        let pid = child.lock().unwrap().id();

        let started = Instant::now();
        let _ = stop_child(&child, pid, Duration::from_millis(200)).unwrap();

        assert!(
            started.elapsed() < Duration::from_secs(10),
            "结束会话不应长时间阻塞"
        );
        assert!(
            try_finish(&child).is_some(),
            "结束会话后进程必须已经退出（否则槽位与状态会被一直占着）"
        );
    }

    // ── 无线链路前置（design D1 / D2）──

    #[test]
    fn only_wireless_devices_get_a_link_attempt() {
        // 冲突点：USB 设备的在线与否是物理事实，`adb connect` 对它没有意义；
        // 若这里判成 Establish，界面会给用户一个注定失败的操作。
        assert_eq!(link_action(config::KIND_WIFI), LinkAction::Establish);
        assert_eq!(link_action(config::KIND_USB), LinkAction::Refuse);
    }

    #[test]
    fn recheck_only_forces_a_link_for_missing_wireless_devices() {
        // 无线设备状态「在线」但实际不在 adb 列表：必须先建链，否则 scrcpy 启动后
        // 以「Could not find any ADB device」退出（第一次失败、第二次成功的根因）。
        assert!(needs_link_before_start(config::KIND_WIFI, false));
        // 实际在列表：不能多跑一次 `adb connect`，那会给每次连接都加上一次子进程。
        assert!(!needs_link_before_start(config::KIND_WIFI, true));
        // USB 两条都必须是 false —— 写反会让离线 USB 也去做连接尝试，违反既有规则。
        assert!(!needs_link_before_start(config::KIND_USB, false));
        assert!(!needs_link_before_start(config::KIND_USB, true));
    }

    #[test]
    fn link_succeeds_only_when_the_reprobe_says_online() {
        // 成功口径是重探测结果，不是 adb 的退出码。
        assert!(link_outcome(Some(device::STATUS_IDLE), "connected to 192.168.1.5:5555").is_ok());

        // adb 报「已连接」但重探测仍非在线（设备 off / unauthorized）→ 仍算失败。
        let err = link_outcome(Some(device::STATUS_OFFLINE), "connected to 192.168.1.5:5555")
            .unwrap_err();
        assert!(err.contains("无法建立无线链路"));

        // 失败原因保留 adb 原文：用户要能区分端口未开、IP 变更与设备端异常。
        let raw = "failed to connect to 192.168.1.5:5555: Connection refused";
        let err = link_outcome(None, raw).unwrap_err();
        assert!(err.contains(raw), "必须保留原始失败原因：{err}");

        // 没有输出时给一句可读的兜底，而不是空原因。
        let err = link_outcome(None, "  ").unwrap_err();
        assert!(err.contains("设备仍不可达"));
    }
}
