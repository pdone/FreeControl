//! 窗口图标：按显示器当前 DPI 从产物内嵌的图标资源里取**原生层**，供给系统的两个图标位置。
//!
//! 不这么做会怎样：运行期默认只把 `.ico` 的**首层**（本仓库是 16×16）设成系统小图标、
//! 大图标位置留空，而任务栏按 32×32（高缩放下更大）取图，只能把 16×16 放大 —— 表现就是图标糊。
//! 这里改成小图标与大图标各自按「16 / 32 × 缩放」向系统要那一档的原生层。
//!
//! 图标资源取自 `tauri-build` 嵌进 exe 的那一份（`bundle.icon` 里的 `.ico`），
//! 与资源管理器看到的是同一份资产，故各处呈现同源。

/// 按窗口所在显示器当前 DPI，把窗口图标供到系统的大图标与小图标两个位置。
///
/// 失败只记一条日志、不阻断启动：图标退回运行期的默认供给，应用照常可用。
#[cfg(windows)]
pub fn apply(window: &tauri::WebviewWindow) {
    if let Err(reason) = apply_for_window(window) {
        crate::logger::warn("app", format!("设置窗口图标失败，沿用默认图标：{reason}"));
    }
}

/// 非 Windows 平台：窗口图标由运行环境自身按平台约定处理（v1 范围外），无对应实现。
#[cfg(not(windows))]
pub fn apply(_window: &tauri::WebviewWindow) {}

#[cfg(windows)]
fn apply_for_window(window: &tauri::WebviewWindow) -> Result<(), String> {
    use std::ffi::c_void;
    use std::ptr;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        LoadImageW, SendMessageW, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_SHARED, WM_SETICON,
    };

    /// `tauri-winres` 把 `bundle.icon` 里的 `.ico` 嵌在 exe 的这个资源 ID 上（见构建期生成的 `resource.rc`）。
    const APP_ICON_RESOURCE_ID: usize = 32512;
    /// 100% 缩放下系统要求的尺寸：大图标供任务栏，小图标供标题栏。
    const LARGE_BASE: f64 = 32.0;
    const SMALL_BASE: f64 = 16.0;

    let hwnd = window.hwnd().map_err(|e| format!("取窗口句柄失败：{e}"))?.0 as *mut c_void;
    let scale = window
        .scale_factor()
        .map_err(|e| format!("取显示器缩放失败：{e}"))?;

    // SAFETY: 传空指针即取当前模块（本 exe）的句柄，只读、无副作用。
    let module = unsafe { GetModuleHandleW(ptr::null()) };
    if module.is_null() {
        return Err("取当前模块句柄失败".into());
    }

    for (base, slot, slot_name) in [
        (LARGE_BASE, ICON_BIG, "大图标"),
        (SMALL_BASE, ICON_SMALL, "小图标"),
    ] {
        let size = (base * scale).round() as i32;
        // SAFETY: 资源 ID 指向本模块内嵌的 RT_GROUP_ICON，系统据此挑最接近的原生层并按需微缩放。
        // `LR_SHARED` 让句柄由系统按「模块 + 资源 + 尺寸」缓存、进程存活期内有效 ——
        // 因此不需要（也不得）对它调 `DestroyIcon`。
        let hicon = unsafe {
            LoadImageW(
                module,
                APP_ICON_RESOURCE_ID as *const u16,
                IMAGE_ICON,
                size,
                size,
                LR_SHARED,
            )
        };
        if hicon.is_null() {
            return Err(format!("取 {size}×{size} 的{slot_name}层失败"));
        }
        // SAFETY: `hwnd` 取自本窗口；`hicon` 由上一行取得，在窗口存活期内一直有效。
        unsafe { SendMessageW(hwnd, WM_SETICON, slot as usize, hicon as isize) };
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    /// 两个图标位置必须都被供给。
    ///
    /// 只设小图标（大图标留空）正是本模块要修的那个缺陷的正源：任务栏拿不到大图标，
    /// 只能把小图标放大。而缺了任一处都不会编译失败、也不会运行报错，只会让图标糊回去，
    /// 故与 `main.rs` 的两条不变量同款做法：直接读本模块源码断言。
    ///
    /// 断言的是**出现次数**而不是「出现过」：常量名在 `use` 那一行里也有一份，
    /// 只看「出现过」的话，把槽位表里的一路删掉仍然通过 —— 那就成了空转。
    /// 每个名字在非测试源码里应出现两次（`use` 一次 + 槽位表一次）。
    #[test]
    fn both_icon_slots_are_supplied() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/window_icon.rs");
        let raw = fs::read_to_string(path).unwrap();
        // 切掉测试部分：下面的断言里也写着这两个名字，不切会自我满足。
        let source = raw.split("#[cfg(test)]").next().unwrap();

        for needle in ["ICON_BIG", "ICON_SMALL"] {
            let seen = source.matches(needle).count();
            assert!(
                seen >= 2,
                "窗口图标必须同时供给大图标与小图标两个位置，但 window_icon.rs 中 {needle} \
                 只出现了 {seen} 次（应至少 2 次：`use` 一次 + 槽位表一次）—— \
                 少的那次说明槽位表里缺了这一路"
            );
        }
    }
}
