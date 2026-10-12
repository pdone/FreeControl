//! 把地址或目录交给系统处理器（design D1–D5）。
//!
//! **为什么在 Windows 上不用 `open` crate**：它不调 ShellExecute，而是先起
//! `powershell.exe -NoProfile -NonInteractive -Command <脚本>`，再**等这个进程退出**
//! （`open-5.4.4/src/lib.rs` 的 `status_without_output()`，crate 文档也自陈
//! "the launcher may block"）。本机实测这段固定开销 **0.49–0.52 s**（同机 `cmd.exe` 11–16 ms），
//! 而调用它的三个命令都是非 async 的 Tauri 命令 —— 函数体就地跑在事件循环线程上，
//! 于是那半秒里窗口不响应、目标也迟迟不出现。
//!
//! `ShellExecuteW` 是一次**进程内**调用：由 shell 把目标交给默认处理器后即返回，没有中间进程，
//! 也没有「等谁退出」的语义；失败仍有返回码可翻成一句可见反馈。
//!
//! 动词与旧行为逐一对齐：
//! - 外链用 `open`：与旧路径的 `Start-Process <url>` 同为「交给默认处理器」；
//! - 目录用 `explore`：与旧路径的 `Invoke-Item <dir>` 同为「在资源管理器里打开该目录**本身**」，
//!   MUST NOT 用 `SHOpenFolderAndSelectItems` 那条 —— 那是「在父目录里选中它」，语义不同。
//!
//! 非 Windows 目标不做这件事的系统 API（v1 范围外），仍交给 `open` crate（Linux 上即 `xdg-open`）。

use std::path::Path;

// ── 对外入口 ─────────────────────────────────────────────

/// 交给系统默认处理器打开一个地址（外链）。
#[cfg(windows)]
pub fn open_url(url: &str) -> Result<(), String> {
    execute("open", url)
}

/// 非 Windows：交给 `open` crate（Linux 上走 `xdg-open`，没有 Windows 那种启动器开销）。
#[cfg(not(windows))]
pub fn open_url(url: &str) -> Result<(), String> {
    open::that(url).map_err(|e| format!("系统未能交给默认处理器（{e}）"))
}

/// 在系统文件管理器中打开一个目录（目录本身，不是「在父目录里选中它」）。
#[cfg(windows)]
pub fn open_dir(dir: &Path) -> Result<(), String> {
    execute("explore", &dir.to_string_lossy())
}

/// 非 Windows：交给 `open` crate。
#[cfg(not(windows))]
pub fn open_dir(dir: &Path) -> Result<(), String> {
    open::that(dir).map_err(|e| format!("系统未能交给默认处理器（{e}）"))
}

// ── Windows：一次 ShellExecuteW ───────────────────────────

#[cfg(windows)]
use std::ffi::OsStr;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

#[cfg(windows)]
use windows_sys::Win32::Foundation::HWND;
#[cfg(windows)]
use windows_sys::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
#[cfg(windows)]
use windows_sys::Win32::UI::Shell::ShellExecuteW;
#[cfg(windows)]
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// `ShellExecuteW` 的返回值大于它就表示成功（MSDN：0–32 是错误码，> 32 才是成功）。
#[cfg(windows)]
const SHELL_EXECUTE_FIRST_SUCCESS_CODE: isize = 33;

#[cfg(windows)]
fn execute(verb: &str, target: &str) -> Result<(), String> {
    ensure_com_initialized();
    let verb = wide(verb);
    let target = wide(target);
    let hwnd: HWND = std::ptr::null_mut();
    // 不传参数与工作目录（两者都是可选），窗口按常规显示。
    let code = unsafe {
        ShellExecuteW(
            hwnd,
            verb.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as isize;
    check_result(code)
}

/// 确保**本线程**已初始化 COM（MSDN 对 `ShellExecute` 的要求，design D5）。
///
/// 这个命令跑在事件循环线程上：它可能已由 tao 的 `OleInitialize` 初始化（STA，这里再调一次返回
/// `S_FALSE`），也可能处于 MTA（返回 `RPC_E_CHANGED_MODE`）—— 两种返回都不影响本次调用，故一律
/// 忽略返回值。正因为「初始化成功的可能不是我们」，这里 MUST NOT 配对调用 `CoUninitialize`。
#[cfg(windows)]
fn ensure_com_initialized() {
    unsafe {
        CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
    }
}

/// 转成以 0 结尾的 UTF-16（Win32 的宽字符串约定）。
#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// 把 `ShellExecuteW` 的返回值翻成结果。
///
/// 纯函数，便于单测 —— 真的调一次会拉起浏览器或资源管理器，不能进单测。
#[cfg(windows)]
fn check_result(code: isize) -> Result<(), String> {
    if code >= SHELL_EXECUTE_FIRST_SUCCESS_CODE {
        return Ok(());
    }
    Err(format!(
        "系统未能交给默认处理器（ShellExecute 返回 {code}：{}）",
        reason(code)
    ))
}

/// 错误码文案取自 MSDN 对 `ShellExecute` 返回值的说明（只翻常见的几个）。
#[cfg(windows)]
fn reason(code: isize) -> &'static str {
    match code {
        0 => "系统资源不足",
        2 => "找不到指定的文件",
        3 => "找不到指定的路径",
        5 => "访问被拒绝",
        8 => "内存不足",
        26 => "共享冲突",
        27 => "文件关联不完整或无效",
        28 => "DDE 操作超时",
        29 => "DDE 事务处理失败",
        30 => "DDE 会话处理失败",
        31 => "没有与该目标关联的处理器",
        _ => "未知原因",
    }
}

// ── 单测（只测纯函数与编码，不真的打开任何东西）─────────────

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// 判据是「> 32 才算成功」：0–32 全部是错误码（32 是 MSDN 里最高的那一个）。
    #[test]
    fn shell_execute_result_is_success_only_above_32() {
        for code in 0..=32 {
            assert!(check_result(code).is_err(), "{code} 应判为失败");
        }
        for code in [33, 42, 128] {
            assert_eq!(check_result(code), Ok(()), "{code} 应判为成功");
        }
    }

    /// 失败信息要能直接展示：带上返回码，并配一句人能读的原因。
    #[test]
    fn failure_message_carries_code_and_reason() {
        let err = check_result(31).unwrap_err();
        assert!(err.contains("31"), "应带上返回码，实际：{err}");
        assert!(
            err.contains("没有与该目标关联的处理器"),
            "应带上原因，实际：{err}"
        );
        assert!(check_result(2).unwrap_err().contains("找不到指定的文件"));
        assert!(check_result(0).unwrap_err().contains("系统资源不足"));
        assert!(check_result(7).unwrap_err().contains("未知原因"));
    }

    /// 宽字符串按 Win32 约定以 0 结尾；非 ASCII 目标同样走 UTF-16。
    #[test]
    fn wide_strings_are_nul_terminated_utf16() {
        assert_eq!(wide("open"), vec![111, 112, 101, 110, 0]);
        let zh = wide("日志");
        assert_eq!(zh.last(), Some(&0), "末尾必须是 0");
        assert_eq!(zh, vec![0x65E5, 0x5FD7, 0]);
    }
}
