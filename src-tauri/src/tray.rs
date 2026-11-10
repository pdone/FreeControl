//! Windows 通知区托盘（app-tray）。
//!
//! 本次只在 Windows 创建托盘；模块边界按平台能力而不是平台名切分：
//! `supported()` / `create()` 是唯一的平台入口，前端只读 `get_tray_status` 的结果。
//! macOS 接菜单栏模板图、Linux 接 appindicator 时，只补这两处的平台实现，
//! 配置字段（`tray.iconStyle` / `tray.closeToTray`）、设置语义与隐藏/退出状态机都不动。
//!
//! 托盘句柄必须放进托管状态保活：`TrayIcon` 被 drop 时图标会从通知区移除。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, Wry};

use crate::config;
use crate::logger;

/// 托盘「退出」事件：前端复用关闭窗口的同一次落盘与退出分支。
pub const EVENT_TRAY_QUIT: &str = "tray-quit-requested";

const MENU_SHOW: &str = "show";
const MENU_QUIT: &str = "quit";

/// 托盘图标 id：退出时凭它把资源表里的句柄取出来，确保图标真正删除。
const TRAY_ID: &str = "main";

/// 前端未在期限内响应托盘退出时的兜底期限。
///
/// 与关闭流程的 800ms 落盘上限同量级；取 1.5s 是为了给「落盘 + 销毁」留出余量，
/// 又不至于让用户觉得点了退出没反应。
const QUIT_WATCHDOG: Duration = Duration::from_millis(1500);

// 三个变体都来自 `assets/icons/tray`，按 Windows 小图标尺寸各备一份原生层：
// 系统把 64px HICON 直接缩到 16px 时走低质量缩放，边缘会出现明显锯齿，
// 所以运行期按主屏缩放在这些层里取最接近的一档，而不是永远给 64px。
// 彩色档用纯色 `#FCCD01` 透明剪影，白 / 黑两档是单色剪影。
const ICON_COLOR: &[(u32, &[u8])] = &[
    (16, include_bytes!("../../assets/icons/tray/tray-16-color.png")),
    (20, include_bytes!("../../assets/icons/tray/tray-20-color.png")),
    (24, include_bytes!("../../assets/icons/tray/tray-24-color.png")),
    (28, include_bytes!("../../assets/icons/tray/tray-28-color.png")),
    (32, include_bytes!("../../assets/icons/tray/tray-32-color.png")),
    (36, include_bytes!("../../assets/icons/tray/tray-36-color.png")),
    (40, include_bytes!("../../assets/icons/tray/tray-40-color.png")),
    (48, include_bytes!("../../assets/icons/tray/tray-48-color.png")),
    (64, include_bytes!("../../assets/icons/tray/tray-64-color.png")),
];
const ICON_WHITE: &[(u32, &[u8])] = &[
    (16, include_bytes!("../../assets/icons/tray/tray-16-white.png")),
    (20, include_bytes!("../../assets/icons/tray/tray-20-white.png")),
    (24, include_bytes!("../../assets/icons/tray/tray-24-white.png")),
    (28, include_bytes!("../../assets/icons/tray/tray-28-white.png")),
    (32, include_bytes!("../../assets/icons/tray/tray-32-white.png")),
    (36, include_bytes!("../../assets/icons/tray/tray-36-white.png")),
    (40, include_bytes!("../../assets/icons/tray/tray-40-white.png")),
    (48, include_bytes!("../../assets/icons/tray/tray-48-white.png")),
    (64, include_bytes!("../../assets/icons/tray/tray-64-white.png")),
];
const ICON_BLACK: &[(u32, &[u8])] = &[
    (16, include_bytes!("../../assets/icons/tray/tray-16-black.png")),
    (20, include_bytes!("../../assets/icons/tray/tray-20-black.png")),
    (24, include_bytes!("../../assets/icons/tray/tray-24-black.png")),
    (28, include_bytes!("../../assets/icons/tray/tray-28-black.png")),
    (32, include_bytes!("../../assets/icons/tray/tray-32-black.png")),
    (36, include_bytes!("../../assets/icons/tray/tray-36-black.png")),
    (40, include_bytes!("../../assets/icons/tray/tray-40-black.png")),
    (48, include_bytes!("../../assets/icons/tray/tray-48-black.png")),
    (64, include_bytes!("../../assets/icons/tray/tray-64-black.png")),
];

/// 用户是否主动把主窗口收进了托盘。
///
/// 这是「隐藏」与「尚未首次呈现」「正在退出」区分的唯一标志：
/// 启动兜底、再次启动的可见性等待、最后设备断开的前台化都读它。
static USER_HIDDEN: AtomicBool = AtomicBool::new(false);

pub fn user_hidden() -> bool {
    USER_HIDDEN.load(Ordering::SeqCst)
}

pub fn set_user_hidden(hidden: bool) {
    USER_HIDDEN.store(hidden, Ordering::SeqCst);
}

/// 窗口真正显示后清除隐藏态：托盘显示、再次启动都经由 `present_main_window`。
pub fn clear_user_hidden() {
    USER_HIDDEN.store(false, Ordering::SeqCst);
}

/// 托盘句柄 + 是否创建成功。
///
/// `ready` 单独记一份而不是每次读 `icon`：命令在多线程上调用，读一个原子量最简单，
/// 也避免为了回答「是否可用」去锁托盘句柄。
#[derive(Default)]
pub struct TrayState {
    icon: Mutex<Option<TrayIcon<Wry>>>,
    ready: AtomicBool,
}

impl TrayState {
    pub fn ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }
}

/// 本次是否提供托盘。macOS / Linux 后续接入时把这里改成对应平台的能力判断。
pub fn supported() -> bool {
    cfg!(windows)
}

/// 创建并常驻托盘。非 Windows 直接返回，不视为错误。
pub fn create(app: &AppHandle, prefs: &config::TrayPrefs, language: &str) -> Result<(), String> {
    if !supported() {
        return Ok(());
    }
    let icon = icon_for(&prefs.icon_style, primary_scale(app))?;
    let (show, quit) = default_labels(language);
    let menu = build_menu(app, &show, &quit)?;
    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("FreeControl")
        .menu(&menu)
        // 左键留给「显示主窗口」，右键才出菜单。
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_menu(app, event.id().as_ref()))
        .on_tray_icon_event(handle_tray_event)
        .build(app)
        .map_err(|e| format!("创建托盘失败：{e}"))?;

    let state = app.state::<TrayState>();
    *state.icon.lock().unwrap() = Some(tray);
    state.ready.store(true, Ordering::SeqCst);
    logger::info("tray", "托盘图标已创建");
    Ok(())
}

/// 进程退出前显式摘掉托盘图标。
///
/// Tauri 的 `run` 在 `RunEvent::Exit` 之后直接 `std::process::exit`，托管状态与资源表都不会
/// 析构，底层托盘收不到删除消息 —— 表现就是退出后图标残留在通知区，鼠标划过才消失。
/// 这里先丢掉自己保存的句柄，再把资源表里那份取出来一并 drop；引用计数归零后才会真正删除。
pub fn dispose(app: &AppHandle) {
    let Some(state) = app.try_state::<TrayState>() else {
        return;
    };
    if !state.ready() {
        return;
    }
    let held = state.icon.lock().unwrap().take();
    drop(held);
    drop(app.remove_tray_by_id(TRAY_ID));
    state.ready.store(false, Ordering::SeqCst);
    logger::info("tray", "托盘图标已移除");
}

/// 切换图标样式。未知取值按彩色处理并记一行 WARN —— 界面只发这三档，
/// 但手工调用命令不该让托盘消失。
pub fn set_icon_style(state: &TrayState, style: &str) -> Result<(), String> {
    let style = known_style(style);
    // 先克隆句柄再离开锁：`set_icon` 可能调度到主线程，不应持锁等待。
    let icon = state
        .icon
        .lock()
        .unwrap()
        .as_ref()
        .cloned()
        .ok_or_else(|| "托盘不可用".to_string())?;
    let image = icon_for(style, primary_scale(icon.app_handle()))?;
    icon.set_icon(Some(image))
        .map_err(|e| format!("切换托盘图标失败：{e}"))?;
    logger::info("tray", format!("托盘图标样式切换为 {style}"));
    Ok(())
}

/// 随界面语言更新原生菜单文案。前端字典是文案的唯一来源，原生菜单只接收结果。
pub fn set_menu_labels(state: &TrayState, show: &str, quit: &str) -> Result<(), String> {
    let show = show.trim();
    let quit = quit.trim();
    if show.is_empty() || quit.is_empty() {
        return Err("托盘菜单文案不能为空".to_string());
    }
    let Some(icon) = state.icon.lock().unwrap().as_ref().cloned() else {
        // 非 Windows / 创建失败：前端仍会照常下发，这里安静跳过。
        return Ok(());
    };
    let menu = build_menu(icon.app_handle(), show, quit)?;
    icon.set_menu(Some(menu))
        .map_err(|e| format!("更新托盘菜单失败：{e}"))
}

fn build_menu(app: &AppHandle, show: &str, quit: &str) -> Result<Menu<Wry>, String> {
    let show_item =
        MenuItem::with_id(app, MENU_SHOW, show, true, None::<&str>).map_err(|e| e.to_string())?;
    let quit_item =
        MenuItem::with_id(app, MENU_QUIT, quit, true, None::<&str>).map_err(|e| e.to_string())?;
    Menu::with_items(app, &[&show_item, &quit_item]).map_err(|e| e.to_string())
}

fn default_labels(language: &str) -> (String, String) {
    if language == config::LANGUAGE_EN {
        ("Show FreeControl".to_string(), "Quit".to_string())
    } else {
        ("显示主界面".to_string(), "退出".to_string())
    }
}

fn known_style(style: &str) -> &str {
    match style {
        config::TRAY_ICON_WHITE => config::TRAY_ICON_WHITE,
        config::TRAY_ICON_BLACK => config::TRAY_ICON_BLACK,
        config::TRAY_ICON_COLOR => config::TRAY_ICON_COLOR,
        other => {
            logger::warn("tray", format!("未知托盘图标样式 {other:?}，按彩色处理"));
            config::DEFAULT_TRAY_ICON
        }
    }
}

fn handle_menu(app: &AppHandle, id: &str) {
    match id {
        MENU_SHOW => {
            if let Err(e) = crate::present_main_window(app, "托盘图标") {
                logger::warn("tray", format!("托盘显示主窗口失败：{e}"));
            }
        }
        MENU_QUIT => request_quit(app),
        other => logger::warn("tray", format!("未知托盘菜单项 {other:?}")),
    }
}

fn handle_tray_event(tray: &TrayIcon<Wry>, event: TrayIconEvent) {
    match event {
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        }
        | TrayIconEvent::DoubleClick {
            button: MouseButton::Left,
            ..
        } => {
            if let Err(e) = crate::present_main_window(tray.app_handle(), "托盘图标") {
                logger::warn("tray", format!("托盘显示主窗口失败：{e}"));
            }
        }
        _ => {}
    }
}

/// 托盘「退出」：先让前端走同一次落盘与退出分支；前端无响应时看门狗直接退出。
fn request_quit(app: &AppHandle) {
    logger::info("tray", "托盘退出：请求退出应用");
    // 先置退出中：落盘到销毁之间再次启动不应把即将退出的窗口拉回前台。
    crate::instance::mark_exiting();
    if let Err(e) = app.emit(EVENT_TRAY_QUIT, ()) {
        logger::warn("tray", format!("托盘退出事件发送失败，直接退出：{e}"));
        app.exit(0);
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(QUIT_WATCHDOG);
        logger::warn("tray", "托盘退出：前端未在期限内响应，直接退出");
        app.exit(0);
    });
}

/// 主屏缩放：通知区挂在主屏任务栏上，托盘图标尺寸不随窗口所在显示器变化。
fn primary_scale(app: &AppHandle) -> f64 {
    app.primary_monitor()
        .ok()
        .flatten()
        .map(|m| m.scale_factor())
        .unwrap_or(1.0)
}

/// 按样式与系统缩放取内嵌 PNG，并解码成托盘图像。
///
/// 系统小图标在 100% 是 16px，150% 是 24px……把 64px 的 HICON 交给系统缩小会走
/// 低质量缩放并出现锯齿，因此这里选最接近 `16 × 缩放` 的原生层。
fn icon_for(style: &str, scale: f64) -> Result<tauri::image::Image<'static>, String> {
    let (variants, label) = match style {
        config::TRAY_ICON_WHITE => (ICON_WHITE, "白色"),
        config::TRAY_ICON_BLACK => (ICON_BLACK, "黑色"),
        _ => (ICON_COLOR, "彩色"),
    };
    let target = (16.0 * scale).round().clamp(16.0, 64.0) as i64;
    let (size, bytes) = variants
        .iter()
        .min_by_key(|item| (item.0 as i64 - target).abs())
        .ok_or_else(|| format!("{label}托盘图标没有可用尺寸"))?;
    let image = tauri::image::Image::from_bytes(bytes)
        .map_err(|e| format!("解码{label}托盘图标失败：{e}"))?;
    logger::info("tray", format!("托盘图标取 {size}px 原生层（目标 {target}px）"));
    Ok(image)
}

// ── Tauri 命令（前端只经能力信号使用，不读平台名）────────────────

#[tauri::command]
pub fn get_tray_status(state: State<'_, TrayState>) -> bool {
    state.ready()
}

#[tauri::command]
pub fn set_tray_icon_style(state: State<'_, TrayState>, style: String) -> Result<(), String> {
    set_icon_style(&state, &style)
}

#[tauri::command]
pub fn set_tray_menu_labels(
    state: State<'_, TrayState>,
    show: String,
    quit: String,
) -> Result<(), String> {
    set_menu_labels(&state, &show, &quit)
}

/// 前端在 `hide()` 成功后调用；`false` 只在隐藏失败回退时使用。
#[tauri::command]
pub fn set_tray_hidden(hidden: bool) {
    set_user_hidden(hidden);
    logger::info(
        "app",
        if hidden {
            "主窗口已收进托盘"
        } else {
            "主窗口隐藏状态已清除"
        },
    );
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// 内容包围盒的宽/高占比与中心占比；`glyph=true` 只算高饱和不透明像素（品牌图形）。
    fn bbox_ratios(px: &[u8], w: u32, h: u32, glyph: bool) -> (f64, f64, f64, f64) {
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (w, 0u32, h, 0u32);
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let r = px[i] as i32;
                let g = px[i + 1] as i32;
                let b = px[i + 2] as i32;
                let sat = r.max(g).max(b) - r.min(g).min(b);
                let hit = if glyph {
                    px[i + 3] > 200 && sat >= 150
                } else {
                    px[i + 3] > 0
                };
                if hit {
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);
                }
            }
        }
        assert!(max_x >= min_x && max_y >= min_y, "托盘图像没有识别到内容");
        (
            (max_x - min_x + 1) as f64 / w as f64,
            (max_y - min_y + 1) as f64 / h as f64,
            (min_x + max_x + 1) as f64 / 2.0 / w as f64,
            (min_y + max_y + 1) as f64 / 2.0 / h as f64,
        )
    }

    #[test]
    fn three_styles_decode_to_distinct_64px_images() {
        let color = icon_for(config::TRAY_ICON_COLOR, 4.0).unwrap();
        let white = icon_for(config::TRAY_ICON_WHITE, 4.0).unwrap();
        let black = icon_for(config::TRAY_ICON_BLACK, 4.0).unwrap();
        for (name, image) in [("彩色", &color), ("白色", &white), ("黑色", &black)] {
            assert_eq!(image.width(), 64, "{name}托盘图标宽度");
            assert_eq!(image.height(), 64, "{name}托盘图标高度");
        }
        assert_ne!(color.rgba(), white.rgba(), "彩色与白色不得是同一份图像");
        assert_ne!(color.rgba(), black.rgba(), "彩色与黑色不得是同一份图像");
        assert_ne!(white.rgba(), black.rgba(), "白色与黑色不得是同一份图像");
    }

    /// 系统请求的小图标尺寸必须取同尺寸原生层，而不是让系统把 64px 缩到 16px。
    ///
    /// 失效方式同样是静默的：图标仍能显示，只是边缘会出现硬阶梯状锯齿。
    #[test]
    fn icon_for_picks_native_size_for_system_scale() {
        for (scale, expected) in [
            (1.0, 16),
            (1.25, 20),
            (1.5, 24),
            (1.75, 28),
            (2.0, 32),
            (2.25, 36),
            (2.5, 40),
            (3.0, 48),
            (4.0, 64),
        ] {
            let image = icon_for(config::TRAY_ICON_COLOR, scale).unwrap();
            assert_eq!(
                (image.width(), image.height()),
                (expected, expected),
                "缩放 {scale} 应取 {expected}px 原生层"
            );
        }
    }

    /// 三档托盘图像必须是完成小尺寸光学校正的透明剪影：占位、居中、彩色为纯色且不铺底板。
    ///
    /// 这些断言读的是编译期嵌入的资产；资产重出后忘记同步，或校正只做了一部分档位，
    /// 编译与运行都不会报错，只有通知区里继续显示偏小的图形、色块或带明暗的彩色档。
    #[test]
    fn tray_images_are_optically_scaled_and_uncropped() {
        let color = icon_for(config::TRAY_ICON_COLOR, 4.0).unwrap();
        let white = icon_for(config::TRAY_ICON_WHITE, 4.0).unwrap();
        let black = icon_for(config::TRAY_ICON_BLACK, 4.0).unwrap();

        for (name, image) in [("彩色", &color), ("白色", &white), ("黑色", &black)] {
            let (w, h, cx, cy) = bbox_ratios(image.rgba(), image.width(), image.height(), false);
            assert!(
                (0.74..=0.82).contains(&w) && (0.80..=0.88).contains(&h),
                "{name}图形占位 {w:.1}%x{h:.1}% 应在 74%–82% / 80%–88%（小尺寸光学校正）"
            );
            assert!(
                (cx - 0.5).abs() <= 0.02 && (cy - 0.5).abs() <= 0.02,
                "{name}图形中心 ({cx:.3}, {cy:.3}) 偏离画布中心超过 2%"
            );
            assert_eq!(
                image.rgba()[3],
                0,
                "{name}档左上角必须透明：托盘三档都不再铺底板"
            );
        }

        let (w, h, _, _) = bbox_ratios(color.rgba(), color.width(), color.height(), true);
        assert!(
            (0.74..=0.82).contains(&w) && (0.80..=0.88).contains(&h),
            "彩色档图形占位 {w:.1}%x{h:.1}% 应在 74%–82% / 80%–88%（小尺寸光学校正）"
        );

        // 彩色档必须是纯色 #FCCD01：不能保留母版的明暗渐变，也不能退回带底板的写法。
        let mut checked = false;
        for px in color.rgba().chunks_exact(4) {
            if px[3] > 250 {
                assert_eq!(
                    (px[0], px[1], px[2]),
                    (252, 205, 1),
                    "彩色托盘图标应为纯色 #FCCD01，实际不透明像素为 {:?}",
                    &px[..3]
                );
                checked = true;
            }
        }
        assert!(checked, "彩色托盘图标里没有不透明像素");
    }

    #[test]
    fn unknown_style_falls_back_to_color() {
        assert_eq!(known_style("purple"), config::TRAY_ICON_COLOR);
        assert_eq!(known_style(""), config::TRAY_ICON_COLOR);
        assert_eq!(known_style(config::TRAY_ICON_WHITE), config::TRAY_ICON_WHITE);
    }

    /// 创建与移除必须使用同一个稳定 id。
    ///
    /// `dispose` 靠 id 从资源表取回句柄；若创建改用无 id 的 builder，取回会静默落空，
    /// 退出后图标重新变回残留状态。
    #[test]
    fn tray_is_registered_with_the_id_used_on_exit() {
        let source = include_str!("tray.rs");
        assert!(
            source.contains("TrayIconBuilder::with_id(TRAY_ID)"),
            "托盘创建必须使用 TRAY_ID，退出时才能按同一 id 取回并删除"
        );
        assert!(
            source.contains("remove_tray_by_id(TRAY_ID)"),
            "退出时必须按 TRAY_ID 移除托盘图标"
        );
    }
}
