//! 主窗口几何的记忆与恢复（spec `app-shell`「窗口的默认尺寸与最小尺寸」）。
//!
//! 保存的是**普通状态**（非最大化、非最小化、非全屏）下的外框位置与内容尺寸，
//! 物理像素。最大化状态不持久化：最大化期间不更新保存值，因此下次以最后一次还原后的
//! 普通几何打开。
//!
//! 写入沿用 `config` 的原子落盘。事件阶段只改内存里的配置，退出时统一写一次，避免
//! 每次移动/缩放都整文件替换。
//!
//! 缩放/移动事件必须**延迟确认**：最大化过程中，系统会先发一次尺寸变化，此时
//! `is_maximized()` 仍可能为 false；立刻记录就会把最大化边界当成普通几何。事件后
//! 等窗口状态稳定再判定，并让后到的事件作废旧任务。

use crate::config::WindowGeometry;
use crate::{config, logger, AppState};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, Window};

/// 最小窗口尺寸，与 `tauri.conf.json` 的 `minWidth` / `minHeight` 保持一致。
pub const MIN_WIDTH: u32 = 1000;
pub const MIN_HEIGHT: u32 = 720;

/// 显示器或保存几何的矩形（物理像素）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// 恢复时的落位方式（纯函数结果）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    /// 保存位置与某个显示器相交：同时恢复尺寸与位置。
    At { width: u32, height: u32, x: i32, y: i32 },
    /// 保存位置已不在任何显示器内：只恢复尺寸，交给系统居中。
    Center { width: u32, height: u32 },
}

impl WindowGeometry {
    fn rect(self) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }
}

/// 两个矩形是否有面积重叠（只是边缘相接不算相交）。
fn intersects(a: &Rect, b: &Rect) -> bool {
    let ax2 = a.x as i64 + a.width as i64;
    let ay2 = a.y as i64 + a.height as i64;
    let bx2 = b.x as i64 + b.width as i64;
    let by2 = b.y as i64 + b.height as i64;
    (a.x as i64) < bx2 && (b.x as i64) < ax2 && (a.y as i64) < by2 && (b.y as i64) < ay2
}

/// 尺寸不低于最小窗口尺寸；损坏的配置（0 或过小）不会把窗口缩没。
fn clamp_size(geometry: WindowGeometry) -> (u32, u32) {
    (
        geometry.width.max(MIN_WIDTH),
        geometry.height.max(MIN_HEIGHT),
    )
}

/// 根据保存几何与当前显示器决定落位方式（纯函数）。
pub fn placement_for(geometry: WindowGeometry, monitors: &[Rect]) -> Placement {
    let (width, height) = clamp_size(geometry);
    if monitors.iter().any(|m| intersects(&geometry.rect(), m)) {
        Placement::At {
            width,
            height,
            x: geometry.x,
            y: geometry.y,
        }
    } else {
        Placement::Center { width, height }
    }
}

/// 当前窗口状态是否适合覆盖保存的普通几何（纯函数）。
///
/// 最大化、最小化与全屏的边界都不是用户选定的普通尺寸；任何一项成立都必须跳过，
/// 否则「最大化后关闭」会把最大化边界当成下次的普通尺寸。
pub fn should_track(maximized: bool, minimized: bool, fullscreen: bool) -> bool {
    !maximized && !minimized && !fullscreen
}

/// 在窗口首次显示前恢复保存的几何。
pub fn restore<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    saved: Option<WindowGeometry>,
) {
    let Some(saved) = saved else {
        return;
    };
    let monitors: Vec<Rect> = window
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| Rect {
            x: m.position().x,
            y: m.position().y,
            width: m.size().width,
            height: m.size().height,
        })
        .collect();
    match placement_for(saved, &monitors) {
        Placement::At {
            width,
            height,
            x,
            y,
        } => {
            if let Err(e) = window.set_size(PhysicalSize::new(width, height)) {
                logger::warn("window", format!("恢复窗口尺寸失败：{e}"));
            }
            if let Err(e) = window.set_position(PhysicalPosition::new(x, y)) {
                logger::warn("window", format!("恢复窗口位置失败：{e}"));
            }
        }
        Placement::Center { width, height } => {
            if let Err(e) = window.set_size(PhysicalSize::new(width, height)) {
                logger::warn("window", format!("恢复窗口尺寸失败：{e}"));
            }
            if let Err(e) = window.center() {
                logger::warn("window", format!("保存的位置不在任何显示器内，居中窗口失败：{e}"));
            }
        }
    }
}

/// 缩放/移动后等待窗口状态稳定的时长；短于最大化/还原的状态切换。
const SETTLE: Duration = Duration::from_millis(250);

/// 最近一次计划中的延迟捕获。后到的事件把序号推高，旧任务醒来后发现序号已变就放弃。
static TRACK_SEQ: AtomicU64 = AtomicU64::new(0);

/// 窗口尺寸/位置变化后延迟捕获：等状态切换结束再读 `is_maximized()`。
pub fn schedule_track<R: tauri::Runtime>(window: &Window<R>) {
    let seq = TRACK_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
    let window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(SETTLE);
        if TRACK_SEQ.load(Ordering::SeqCst) == seq {
            track(&window);
        }
    });
}

/// 把窗口当前的普通几何写进内存配置；退出时再由 [`persist`] 落盘。
pub fn track<R: tauri::Runtime>(window: &Window<R>) {
    let maximized = window.is_maximized().unwrap_or(false);
    let minimized = window.is_minimized().unwrap_or(false);
    let fullscreen = window.is_fullscreen().unwrap_or(false);
    if !should_track(maximized, minimized, fullscreen) {
        return;
    }
    let (Ok(size), Ok(position)) = (window.inner_size(), window.outer_position()) else {
        return;
    };
    let Some(state) = window.app_handle().try_state::<AppState>() else {
        // 窗口事件可能早于 `setup` 里的 `manage`；此时只跳过，不影响启动。
        return;
    };
    let geometry = WindowGeometry {
        width: size.width,
        height: size.height,
        x: position.x,
        y: position.y,
    };
    let mut cfg = state.config.lock().unwrap();
    if cfg.window != Some(geometry) {
        cfg.window = Some(geometry);
    }
}

/// 退出时把含最新几何的整份配置原子写盘。
pub fn persist(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let cfg = state.config.lock().unwrap().clone();
    // 失败只记日志（`save_config_to_disk` 内部已记），不阻塞退出。
    let _ = config::save_config_to_disk(app, &cfg);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geom(width: u32, height: u32, x: i32, y: i32) -> WindowGeometry {
        WindowGeometry {
            width,
            height,
            x,
            y,
        }
    }

    fn monitor(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn overlapping_geometry_is_restored_at_its_position() {
        let saved = geom(1280, 800, 120, 80);
        let got = placement_for(saved, &[monitor(0, 0, 1920, 1080)]);
        assert_eq!(
            got,
            Placement::At {
                width: 1280,
                height: 800,
                x: 120,
                y: 80
            }
        );
    }

    #[test]
    fn fully_offscreen_geometry_centers_with_saved_size() {
        let saved = geom(1280, 800, -3000, 200);
        let got = placement_for(saved, &[monitor(0, 0, 1920, 1080)]);
        assert_eq!(
            got,
            Placement::Center {
                width: 1280,
                height: 800
            }
        );
    }

    #[test]
    fn partially_offscreen_geometry_still_counts_as_visible() {
        // 右上角只露出 10px：仍与显示器相交，不应被当作离屏。
        let saved = geom(1280, 800, 1910, 0);
        let got = placement_for(saved, &[monitor(0, 0, 1920, 1080)]);
        assert_eq!(
            got,
            Placement::At {
                width: 1280,
                height: 800,
                x: 1910,
                y: 0
            }
        );
    }

    #[test]
    fn touching_edges_do_not_count_as_overlap() {
        let saved = geom(1100, 720, 1920, 0);
        let got = placement_for(saved, &[monitor(0, 0, 1920, 1080)]);
        assert_eq!(got, Placement::Center { width: 1100, height: 720 });
    }

    #[test]
    fn tiny_saved_size_is_clamped_to_minimum() {
        let saved = geom(10, 10, 0, 0);
        let got = placement_for(saved, &[monitor(0, 0, 1920, 1080)]);
        assert_eq!(
            got,
            Placement::At {
                width: MIN_WIDTH,
                height: MIN_HEIGHT,
                x: 0,
                y: 0
            }
        );
    }

    #[test]
    fn any_monitor_can_hold_the_saved_geometry() {
        let saved = geom(1280, 800, 2500, 100);
        let got = placement_for(
            saved,
            &[monitor(0, 0, 1920, 1080), monitor(1920, 0, 1920, 1080)],
        );
        assert!(matches!(got, Placement::At { x: 2500, .. }));
    }

    #[test]
    fn maximized_minimized_or_fullscreen_is_not_tracked() {
        assert!(should_track(false, false, false));
        assert!(!should_track(true, false, false));
        assert!(!should_track(false, true, false));
        assert!(!should_track(false, false, true));
    }
}
