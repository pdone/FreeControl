//! 单实例的进程侧逻辑：再次启动的激活、退出中的抑制，以及更新重启的实例交接。
//!
//! 实例身份与第二进程的判定由 `tauri-plugin-single-instance` 承担（Windows 命名互斥 /
//! Linux D-Bus / macOS 本地 socket）。本模块只处理「判定为已有实例之后」与「重启新进程自己
//! 等旧实例」这两段：前者把已有主窗口呈现出来，后者保证更新重启不会被单例判定拦死。

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
#[cfg(unix)]
use std::time::Instant;

use tauri::Manager;

use crate::logger;

/// 再次启动的轮询步长。
const SECOND_LAUNCH_STEP: Duration = Duration::from_millis(100);
/// 等主窗口出现、等日志初始化：各约 2 s。
const READY_STEPS: u32 = 20;
/// 等前端首帧揭示：与启动兜底的绝对上限同量级（10 s）。
///
/// 主窗口以隐藏方式创建，由前端在首帧后 `show()`。直接 `show()` 会先把一块纯色空窗体放上屏，
/// 违反「窗口出现时机」；以可见性作为就绪判据即可复用前端已有的 FCP 门槛，不必新增 IPC。
/// 上限取 10 s 而不是更短：冷启动时 WebView 首帧可能晚于 2 s，提前放行会把纯色窗体现出来。
const REVEAL_STEPS: u32 = 100;

/// 更新重启时等旧进程退出的上限。
///
/// 旧实例在 `spawn` 之前已经保存配置并结束了全部会话，正常情况下退出只需毫秒级；10 s
/// 与单例插件在 Windows 上转发消息的超时同量级，既不会把「旧进程卡死」变成无限等待，
/// 也不会因为一次系统卡顿就误判。
pub const RELAUNCH_WAIT: Duration = Duration::from_secs(10);

/// 内部参数：由 `app_update::replace_and_restart` 传给新版本进程，值为旧进程 pid。
const RELAUNCH_FLAG: &str = "--relaunch-after";

/// 应用是否已进入退出流程。
///
/// 前端在关窗时先拦截 `close-requested` 落盘、再 `destroy()`；这段落盘期间窗口仍然存在，
/// 单例回调若只看「窗口是否存在」会把即将退出的窗口又拉回前台。故在 `CloseRequested`
/// 到达时先行置位；`RunEvent::ExitRequested`（macOS 的 Cmd+Q 等不经过关窗事件的路径）
/// 也同样置位。
static EXITING: AtomicBool = AtomicBool::new(false);

pub fn mark_exiting() {
    EXITING.store(true, Ordering::SeqCst);
}

pub fn is_exiting() -> bool {
    EXITING.load(Ordering::SeqCst)
}

/// 第二个进程被判定为「已有实例」后的回调。
///
/// 这里只做三件事：记下启动参数与工作目录（当前没有 CLI 语义，留作排查）、检查是否正在退出、
/// 把已有主窗口呈现到前台。**不**创建窗口、不加载配置、不执行启动清理 —— 它们都在
/// `app.setup` 中，而插件判定发生在其之前，第二进程根本走不到那里。
pub fn on_second_launch<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    args: Vec<String>,
    cwd: String,
) {
    if is_exiting() {
        logger::info("app", "再次启动：首个实例正在退出，忽略本次激活");
        return;
    }
    // 各平台插件的回调线程不同：Linux 直接跑在 D-Bus 监听线程上，可能早于窗口创建与
    // 日志初始化。统一挪到后台线程等待，既避免阻塞监听线程，也把「冷启动期间再次启动」
    // 的激活与留痕补回来。
    let app = app.clone();
    std::thread::spawn(move || {
        let mut window = None;
        for _ in 0..READY_STEPS {
            if is_exiting() {
                return;
            }
            if let Some(found) = app.get_webview_window("main") {
                window = Some(found);
                break;
            }
            std::thread::sleep(SECOND_LAUNCH_STEP);
        }
        let Some(window) = window else {
            logger::warn("app", "再次启动：未找到主窗口，无法呈现");
            return;
        };
        // 窗口在 setup 中创建，日志初始化紧随其后；等它就绪，避免本次启动的留痕被静默丢弃。
        for _ in 0..READY_STEPS {
            if logger::is_initialized() {
                break;
            }
            std::thread::sleep(SECOND_LAUNCH_STEP);
        }
        logger::info(
            "app",
            format!("再次启动：启动参数 {args:?}，工作目录 {cwd}"),
        );
        if is_exiting() {
            return;
        }

        let mut visible = window.is_visible().unwrap_or(false);
        if !visible {
            logger::info("app", "再次启动：主窗口尚未显示，等界面就绪后再呈现");
        }
        for _ in 0..REVEAL_STEPS {
            if visible {
                break;
            }
            if is_exiting() {
                return;
            }
            std::thread::sleep(SECOND_LAUNCH_STEP);
            visible = window.is_visible().unwrap_or(false);
        }
        if is_exiting() {
            return;
        }
        present(&app, "再次启动");
    });
}

fn present<R: tauri::Runtime>(app: &tauri::AppHandle<R>, reason: &str) {
    if let Err(e) = crate::present_main_window(app, reason) {
        logger::warn("app", format!("{reason}：呈现主窗口失败：{e}"));
    }
}

/// 从参数里取更新重启的「等待目标」pid。
///
/// 只认 `--relaunch-after <pid>` 这一种写法，且只在恰好出现一次、取值是正数时生效：
/// 缺失、非数字、重复出现都按「没有这个参数」处理，照常启动 —— 它只是应用内部的重启交接，
/// 不该让一个残缺参数改变正常启动路径。
pub fn relaunch_after<I: IntoIterator<Item = String>>(args: I) -> Option<u32> {
    let mut iter = args.into_iter();
    let mut found = None;
    while let Some(arg) = iter.next() {
        if arg != RELAUNCH_FLAG {
            continue;
        }
        let Some(value) = iter.next() else {
            return None;
        };
        let Ok(pid) = value.parse::<u32>() else {
            return None;
        };
        if pid == 0 || found.is_some() {
            return None;
        }
        found = Some(pid);
    }
    found
}

/// 等目标进程结束；返回 `true` 表示已结束（或已无法观察），`false` 表示期限内仍在运行。
///
/// Windows 直接等待进程句柄；Unix 用 `kill(pid, 0)` 探测。两者都以「查不到即视为已退出」
/// 为语义：更新重启只关心旧实例是否已经释放实例身份，不关心它退出的具体原因。
#[cfg(windows)]
pub fn wait_for_process_exit(pid: u32, timeout: Duration) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
    };

    let millis = timeout.as_millis().min(u32::MAX as u128) as u32;
    unsafe {
        let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if handle.is_null() {
            return true;
        }
        let code = WaitForSingleObject(handle, millis);
        CloseHandle(handle);
        code == WAIT_OBJECT_0
    }
}

#[cfg(unix)]
pub fn wait_for_process_exit(pid: u32, timeout: Duration) -> bool {
    let pid = pid as libc::pid_t;
    let deadline = Instant::now() + timeout;
    loop {
        let alive = unsafe { libc::kill(pid, 0) } == 0
            || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM);
        if !alive {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn relaunch_after_accepts_a_single_positive_pid() {
        assert_eq!(relaunch_after(args(&["--relaunch-after", "1234"])), Some(1234));
        assert_eq!(
            relaunch_after(args(&["--other", "x", "--relaunch-after", "42"])),
            Some(42)
        );
    }

    #[test]
    fn relaunch_after_ignores_missing_or_invalid_values() {
        assert_eq!(relaunch_after(args(&["--relaunch-after"])), None);
        assert_eq!(relaunch_after(args(&["--relaunch-after", "abc"])), None);
        assert_eq!(relaunch_after(args(&["--relaunch-after", "-1"])), None);
        assert_eq!(relaunch_after(args(&["--relaunch-after", "0"])), None);
        assert_eq!(relaunch_after(args(&["--relaunch-after", "--x"])), None);
        assert_eq!(relaunch_after(args(&[])), None);
    }

    #[test]
    fn relaunch_after_ignores_duplicates() {
        assert_eq!(
            relaunch_after(args(&[
                "--relaunch-after",
                "1",
                "--relaunch-after",
                "2"
            ])),
            None
        );
    }

    /// 已退出的子进程应被立即判定为结束。
    #[test]
    fn wait_returns_immediately_for_exited_process() {
        #[cfg(windows)]
        let mut child = std::process::Command::new("cmd")
            .args(["/C", "exit", "0"])
            .spawn()
            .expect("spawn cmd");
        #[cfg(unix)]
        let mut child = std::process::Command::new("sh")
            .args(["-c", "exit 0"])
            .spawn()
            .expect("spawn sh");

        let pid = child.id();
        let _ = child.wait();
        assert!(wait_for_process_exit(pid, Duration::from_secs(5)));
    }

    /// 目标进程仍在运行时，等待应以超时结束。
    #[test]
    fn wait_times_out_for_running_process() {
        assert!(!wait_for_process_exit(
            std::process::id(),
            Duration::from_millis(100)
        ));
    }
}
