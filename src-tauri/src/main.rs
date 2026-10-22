// 发布构建为图形子系统：Windows 上不额外分配控制台窗口，启动时只呈现主窗口。
// 调试构建保留控制台是有意的 —— 开发期要直接看到编译与运行期输出，不必翻日志文件。
// 移除本行会让发布产物多出一个空控制台窗口，且编译、单测、出包全都不会失败。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod scrcpy;
mod config;
mod device;
mod assets;
mod app_update;
mod download;
mod session;
mod links;
mod logger;
mod shell;
mod ui_message;
mod window_icon;
mod webview_env;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::Manager;

/// 应用级可变状态：当前配置（含设备列表、默认参数、覆盖、通道偏好）。
/// 配置文件在 `setup` 阶段从磁盘加载，之后读写均经由此状态，并去抖落盘。
///
/// `pending` 保存「已下载但未经官方校验、等待用户确认」的安装中间态（tasks 4.5），
/// 使用户确认后无需重新下载；被替换或取消时其中的临时文件会被清掉。
///
/// `install` 保存**在途安装**的人工控制入口（design D2）：命令层借它把「切换通道 / 取消」
/// 送进正在跑的下载循环。每次安装新建一份、结束时置回 `None`，因此不存在陈旧信号；
/// 用 `Arc` 是因为循环里要持有它而不长期锁住这个 `Mutex`。
#[derive(Default)]
pub struct AppState {
    pub config: Mutex<config::AppConfig>,
    pub pending: Mutex<Option<download::PendingInstall>>,
    /// 在途安装的控制入口；`None` 表示当前没有进行中的获取。
    pub install: Mutex<Option<Arc<download::InstallCtl>>>,
    /// 会话表（design D13 的「会话层」）：设备 id → 会话记录（进程句柄、日志、窗口槽位）。
    ///
    /// 记录里 `exit` 为 `None` 表示进程仍在运行 —— `device::merge_statuses` 只把这类算作
    /// 「已连接」，而槽位分配只考虑这类会话的占用。已结束的记录**保留**在表里，供「更多」页
    /// 查看会话输出（tasks 9.4），数量由 `session::MAX_ENDED_SESSIONS` 限制。
    pub sessions: Mutex<HashMap<String, session::SessionRecord>>,
    /// 最近一次设备状态探测结果（tasks 6.3 / 9.1）。
    ///
    /// 放在状态里而不是订阅线程的局部变量：连接 / 断开后要立即重算并推送设备状态，
    /// 不能让界面干等下一次设备变化通知。
    pub adb_probe: Mutex<device::AdbProbe>,
    /// 最近一次计算出的设备状态快照（tasks 6.3）：事件推送与 `get_device_status`
    /// 命令共用同一份数据，避免「首次推送早于前端注册监听」造成界面永远停在检测中；
    /// 同时充当「变化才推送」的比较基准。
    pub device_status: Mutex<Option<device::DeviceStatusPayload>>,
    /// 本次运行内已确认「无法建立连接」的下载通道（design D1）。
    ///
    /// 纯内存态、**不写入配置文件**：位置本身就保证每次启动仍以官方通道为第一个尝试对象。
    /// 一次下载成功完成后清空（`download::finish_install`）。
    pub channel_memory: Mutex<download::ChannelMemory>,
    /// 上次检查确定的应用更新下载计划（版本 / 资产 / 地址 / 校验值）。
    ///
    /// 与运行环境的 `install` 完全分开：两者可以各自独立进行，互不覆盖状态。
    pub app_update_plan: Mutex<Option<app_update::AppUpdatePlan>>,
    /// 已下载并校验通过、等待「立即重启并更新」的包。
    pub app_update_pending: Mutex<Option<app_update::PendingAppUpdate>>,
    /// 在途应用更新下载的控制入口；`None` 表示当前没有下载。
    pub app_update_ctl: Mutex<Option<Arc<download::InstallCtl>>>,
}

/// 兜底显示主窗口的两条期限（取值理由见 `spawn_reveal_fallback`）。
const FALLBACK_REVEAL_AFTER_LOAD: Duration = Duration::from_millis(800);
const FALLBACK_REVEAL_AFTER_START: Duration = Duration::from_millis(10_000);

/// 期限到达后，若主窗口仍未显示则显示它，并记一条 WARN。
///
/// 主窗口以**隐藏方式**创建（`tauri.conf.json` 的 `visible: false`），正常由前端在挂载完成后
/// 调用 `show()` 放出来（`src/lib/store.ts::revealWindow`）。前端一旦失效 —— 脚本加载失败、
/// 挂载异常、或能力声明漏配 `core:window:allow-show` —— 窗口就永远不会出现，故必须有兜底：
/// - `FALLBACK_REVEAL_AFTER_LOAD` 自**页面加载完成**起算：锚在 load 上而不是进程启动，
///   冷启动（WebView2 首次初始化明显更慢）不会被误触发；覆盖「页面加载完了却没显示窗口」。
/// - `FALLBACK_REVEAL_AFTER_START` 自进程启动起算的**绝对上限**，覆盖「页面始终没加载完」
///   （打包 / 资源事故）—— 那一路连 load 都不会来。
///
/// 计时走独立线程：本函数在 Tauri 的事件回调里被调用，睡在这里会把界面卡住。
/// 是否要显示由 `is_visible` 判定，重复触发无害（前端已经显示成功就直接返回）。
/// 真正走到显示那一步时记 WARN —— 否则「启动莫名变慢」只能靠猜。
fn spawn_reveal_fallback(app: tauri::AppHandle, delay: Duration, reason: &'static str) {
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        let Some(window) = app.get_webview_window("main") else {
            return;
        };
        let visible = window.is_visible();
        if matches!(visible, Ok(true)) {
            return;
        }
        logger::warn(
            "app",
            format!("前端未在期限内显示窗口（{reason}），改由兜底显示（is_visible={visible:?}）"),
        );
        if let Err(e) = window.show() {
            logger::warn("app", format!("兜底显示窗口失败：{e}"));
        }
    });
}

/// 把主窗口显示到前台（app-shell 规格「最后一个设备离开已连接状态后主窗口回到前台」）。
///
/// **为什么在 Rust 侧**：窗口调用若由前端发起，`unminimize` / `set-focus` 都得逐项写进
/// `capabilities/default.json`（ACL 只约束前端 IPC），这里不需要 —— 少两项权限；而且前端
/// 没有写日志的通道，而「前台化结果留痕」正是这条行为在真机上唯一的复核出口（release 产物里
/// 看不到 `console.error`）。
///
/// **两步尝试，MUST NOT 只请求焦点就作罢**：
/// 1. `show()`（应对隐藏态）→ `unminimize()`（应对最小化态）→ `set_focus()`。
///    实测：`unminimize` 只对**已最小化**的窗口有激活作用，窗口没最小化时它对前台毫无帮助。
/// 2. 若仍未取得焦点 —— Windows 的**前台锁**：调用进程既不是前台进程、也没收到最后一次输入时，
///    `SetForegroundWindow` 会被**静默拒绝**（系统只闪一下任务栏按钮，不抛错 —— 正是用户
///    「点完断开就切去浏览器、几秒后会话才结束」时看到的现象）—— 改用 **topmost 翻转**：
///    置顶是 z 序操作、不受前台锁约束，窗口必定画在普通窗口之上；复原走的是 `HWND_NOTOPMOST`，
///    平台语义是「放到所有非置顶窗口之上」，因此复原后窗口仍停在最前，而不是掉回原处。
///
/// 取得焦点只作**尽力而为**：前台锁是系统「不打断用户当前操作」的设计，本命令保证的是**看得见**。
/// 结果写日志（取得焦点 INFO / 只做到可见 WARN），复核时一眼看出走了哪条分支。
#[tauri::command]
fn bring_main_window_to_front(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "未找到主窗口".to_string())?;
    window
        .show()
        .map_err(|e| format!("显示主窗口失败：{e}"))?;
    if let Err(e) = window.unminimize() {
        // 窗口没最小化时这一步没有意义，失败也不影响后续 —— 记一行即可。
        logger::warn("app", format!("主窗口恢复（unminimize）失败：{e}"));
    }
    let focused = focus_if_possible(&window);
    let mut raised = false;
    if !focused {
        // 前台锁拒了：先把窗口抬到最前（z 序，不受前台锁约束），再试一次取焦点。
        match window.set_always_on_top(true) {
            Ok(()) => {
                raised = true;
                let _ = window.set_focus();
                if let Err(e) = window.set_always_on_top(false) {
                    logger::warn("app", format!("主窗口取消置顶失败：{e}"));
                }
            }
            Err(e) => logger::warn("app", format!("主窗口置顶兜底失败：{e}")),
        }
    }
    let focused_now = window.is_focused().unwrap_or(false);
    logger::log(
        if focused_now {
            logger::LEVEL_INFO
        } else {
            logger::LEVEL_WARN
        },
        "app",
        &format!("主窗口前台化（最后一个设备已断开）：focused={focused_now}，置顶兜底={raised}"),
    );
    Ok(())
}

/// 请求前台焦点并回报是否**真的**拿到。
///
/// 被前台锁拒绝时 `set_focus()` 不报错（系统只闪任务栏按钮），所以只能以 `is_focused()` 为准。
fn focus_if_possible(window: &tauri::WebviewWindow) -> bool {
    window.set_focus().is_ok() && window.is_focused().unwrap_or(false)
}

/// 关掉主窗口的系统装饰（Windows），使界面顶部那条自绘标题栏成为窗口唯一的标题栏。
///
/// **为什么在 Rust 侧按平台做，而不是写进平台专属配置**（`tauri.windows.conf.json`）：
/// Tauri 合并平台配置用的是 `json_patch::merge`，即 RFC 7386 的 merge patch —— **数组整体替换**
/// （patch 的值为数组时直接取 patch）。窗口定义在 `app.windows` 数组里，于是平台文件必须整份复制
/// 窗口定义，否则 `visible` / 尺寸 / `backgroundColor` 会回落到默认值（`visible` 默认 `true`，
/// 窗口会在界面就绪前上屏，直接破坏「窗口出现时机」与「首帧与窗口背景」）。整份复制等于给窗口几何
/// 立第二个事实源。这里一行反而保持 `tauri.conf.json` 为唯一来源 —— 与本仓既有的单一来源约定一致。
///
/// **时机**：在 `setup` 中调用，窗口此时**尚未显示**（`visible: false`，由前端的就绪信号放出来），
/// 因此边框的这次变化不会出现在屏幕上。
///
/// **与前端的分工**：界面依据 `isDecorated()` 的**事实**决定是否呈现那三枚窗口控件
/// （`src/lib/store.ts`），不在前端另写一份平台清单；两边都读同一个窗口状态。
fn disable_system_decorations(window: &tauri::WebviewWindow) {
    #[cfg(windows)]
    if let Err(e) = window.set_decorations(false) {
        logger::warn("app", format!("关闭窗口装饰失败，将保留系统标题栏：{e}"));
    }
    #[cfg(not(windows))]
    let _ = window;
}

fn main() {
    // Linux：按需关掉 WebKit 的 DMA-BUF 渲染器（本变更 design D12）。必须早于 `tauri::Builder`
    // —— 窗口在 `setup` 之前创建、web 进程更晚，而 WebKit 是在自身初始化时读这个变量，之后再设
    // 就没用了。判定所需的读操作失败一律退化为「该信号未命中」，不影响启动（见 `webview_env`）。
    webview_env::prepare();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // 兜底之一：页面加载完成后仍未显示窗口（锚点选择见 `spawn_reveal_fallback`）。
        .on_page_load(|webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                spawn_reveal_fallback(
                    webview.app_handle().clone(),
                    FALLBACK_REVEAL_AFTER_LOAD,
                    "页面加载完成后仍未显示窗口",
                );
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();
            let loaded = config::load_config(&handle);
            let cfg = loaded.clone().unwrap_or_default();
            if let Ok(root) = config::app_data_root(&handle) {
                // 日志落盘（design D12）：必须在配置加载之后 —— 保留策略来自配置；
                // 也必须早于其余清理，使失败原因也能进日志。
                logger::init(&root, cfg.logging.retention_days());
                // 上一行判定才落盘：`prepare()` 早于日志初始化，而日志未初始化时写入被静默丢弃
                // （design D12 的留痕）。
                webview_env::log_decision();
                if let Err(e) = &loaded {
                    logger::warn("config", format!("读取配置失败，按出厂默认启动：{e}"));
                }
                // 清理上次运行可能残留的临时文件：下载临时目录（tasks 4.3）与写配置的临时文件（tasks 5.2）。
                download::cleanup_stale_staging(&root);
                // 自定义目录的临时目录建在它的父目录下（与目标同卷），启动时一并清理。
                if let Ok(effective) =
                    scrcpy::resolve_scrcpy_dir(&root, cfg.scrcpy_dir.as_deref())
                {
                    if let Some(parent) = effective.parent() {
                        if parent != root {
                            download::cleanup_stale_staging(parent);
                        }
                    }
                }
                config::cleanup_stale_tmp(&root);
                // 应用更新：未应用的待更新包与替换时的旧版本备份只在本次运行内有效。
                app_update::cleanup_stale(&handle);
            }
            // 窗口图标按显示器 DPI 供给原生层（design D1）。放在这里的前提是 config 声明的窗口
            // 已先于本钩子创建（见 tauri 的 `app::setup`），此时窗口句柄才有效。
            for (_, window) in app.webview_windows() {
                window_icon::apply(&window);
                // 同上：窗口已创建、尚未显示，正是关装饰的时机（见该函数的说明）。
                disable_system_decorations(&window);
            }
            app.manage(AppState {
                config: Mutex::new(cfg),
                pending: Mutex::new(None),
                install: Mutex::new(None),
                sessions: Mutex::new(HashMap::new()),
                adb_probe: Mutex::new(device::AdbProbe::default()),
                device_status: Mutex::new(None),
                channel_memory: Mutex::new(download::ChannelMemory::default()),
                app_update_plan: Mutex::new(None),
                app_update_pending: Mutex::new(None),
                app_update_ctl: Mutex::new(None),
            });
            // 兜底之二：自进程启动起算的绝对上限（见 `spawn_reveal_fallback`）。
            spawn_reveal_fallback(
                handle.clone(),
                FALLBACK_REVEAL_AFTER_START,
                "启动后仍未显示窗口",
            );
            // 设备状态订阅线程（tasks 6.3）：必须在 `manage` 之后启动 —— 它要读状态。
            device::spawn_status_subscriber(handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scrcpy::get_compat_status,
            assets::get_scrcpy_dir,
            assets::get_env_status,
            assets::open_scrcpy_dir,
            assets::set_scrcpy_dir,
            config::get_config,
            config::save_config,
            device::get_devices,
            device::get_device_status,
            device::add_device,
            device::update_device,
            device::delete_device,
            session::connect_device,
            session::connect_all,
            session::disconnect_device,
            session::disconnect_all,
            session::list_sessions,
            session::get_session_log,
            session::clear_session_log,
            download::get_proxy_list,
            download::add_proxy_address,
            download::remove_proxy_address,
            download::check_update,
            download::start_scrcpy_install,
            download::confirm_scrcpy_install,
            download::cancel_scrcpy_install,
            download::switch_scrcpy_channel,
            app_update::check_app_update,
            app_update::start_app_update,
            app_update::cancel_app_update,
            app_update::apply_app_update,
            links::open_external_link,
            logger::open_log_dir,
            bring_main_window_to_front,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|handle, event| {
            // 会话由本应用持有：退出前结束它们，否则会留下无人管理、也无人能断开的投屏窗口。
            if let tauri::RunEvent::Exit = event {
                session::shutdown_all(handle);
                // adb 服务端**不**在退出时结束：它与其设备连接留给下次启动复用，退出清场对
                // 「下载最新版本」毫无贡献 —— 替换前那一刻会自行结束它（design D1）。
                // 退出行写在各会话的结束行**之后**：顺序反了会把「会话被应用主动结束」
                // 记在「应用退出」之后，时间线读起来是错的（design D12）。
                logger::info("app", "应用退出");
            }
        });
}

#[cfg(test)]
mod tests {
    //! 配置不变量。这些声明一旦缺失，失效方式是**静默的**：功能不报错，只是不生效，
    //! 排查成本远高于写这条断言。

    use serde_json::Value;

    /// 能力声明必须授予窗口 `destroy` / `close`。
    ///
    /// `store.ts::registerFlushOnClose` 会拦下 `close-requested` 去刷未落盘的配置，
    /// 刷完再 `win.destroy()`；而 `destroy` / `close` **都不在** `core:default` 里。
    /// 缺权限时 `preventDefault()` 生效、销毁被拒 → 「点标题栏关闭按钮没反应、程序也不退出」。
    #[test]
    fn capability_grants_window_close_permissions() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/capabilities/default.json");
        let raw = std::fs::read_to_string(path).unwrap();
        let json: Value = serde_json::from_str(&raw).unwrap();
        let perms: Vec<String> = json["permissions"]
            .as_array()
            .expect("capabilities 必须有 permissions 数组")
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();

        for needed in ["core:window:allow-destroy", "core:window:allow-close"] {
            assert!(
                perms.iter().any(|p| p == needed),
                "关窗流程依赖 {needed}，但 capabilities/default.json 未声明：{:?}",
                perms
            );
        }
    }

    /// 能力声明必须授予窗口 `show`。
    ///
    /// 主窗口以隐藏方式创建（`tauri.conf.json` 的 `visible: false`），由前端在挂载完成后
    /// `show()` 放它出来；而 `core:default` 只包含窗口的**只读**命令，`show` 不在其中。
    /// 缺权限时前端渲染一切正常、只有 `show()` 被拒 —— 表现为「每次启动都静默多等一次兜底
    /// 期限（800ms）」，界面本身看不出任何异常，属最难定位的一类失效。
    ///
    /// 「断开最后一个设备时把主窗口显示到前台」**不走前端**：它由本文件里的自有命令
    /// `bring_main_window_to_front` 完成（窗口调用在 Rust 侧不受 ACL 约束），
    /// 故 `unminimize` / `set-focus` 不需要在这里声明。
    #[test]
    fn capability_grants_window_show_permission() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/capabilities/default.json");
        let raw = std::fs::read_to_string(path).unwrap();
        let json: Value = serde_json::from_str(&raw).unwrap();
        let perms: Vec<String> = json["permissions"]
            .as_array()
            .expect("capabilities 必须有 permissions 数组")
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();

        assert!(
            perms.iter().any(|p| p == "core:window:allow-show"),
            "启动期放窗口出来依赖 core:window:allow-show，但 capabilities/default.json 未声明：{:?}",
            perms
        );
    }

    /// 能力声明必须授予自绘标题栏用到的三条窗口命令：`start-dragging` / `minimize` / `toggle-maximize`。
    ///
    /// 三者都**不在** `core:default` 里（该默认集只有窗口的只读命令 + 双击最大化的那条内部命令），
    /// 缺任一条的失效方式都是**静默的**：拖标题栏拖不动、点最小化 / 最大化没反应，界面本身看不出
    /// 任何异常。与上两条同性质，故沿用同一做法直接断言能力清单。
    ///
    /// 关闭键不需要新权限：它走既有的 `close`（已被上面那条断言覆盖）。
    #[test]
    fn capability_grants_titlebar_control_permissions() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/capabilities/default.json");
        let raw = std::fs::read_to_string(path).unwrap();
        let json: Value = serde_json::from_str(&raw).unwrap();
        let perms: Vec<String> = json["permissions"]
            .as_array()
            .expect("capabilities 必须有 permissions 数组")
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();

        for needed in [
            "core:window:allow-start-dragging",
            "core:window:allow-minimize",
            "core:window:allow-toggle-maximize",
        ] {
            assert!(
                perms.iter().any(|p| p == needed),
                "自绘标题栏依赖 {needed}，但 capabilities/default.json 未声明：{:?}",
                perms
            );
        }
    }

    /// 发布构建必须声明为 Windows 图形子系统。
    ///
    /// 缺失该属性时，发布产物的 PE 子系统回落为控制台（`Subsystem = 3`），Windows 会在主窗口之外
    /// 多分配一个空控制台窗口 —— 而编译、单测、出包全都不会失败，只有用户会看到那个黑框。
    /// 故与上一条同款做法：直接读源码断言这条不变量。
    #[test]
    fn release_build_declares_windows_gui_subsystem() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs");
        let raw = std::fs::read_to_string(path).unwrap();
        // 归一化连续空白：改缩进或换行不得误报。
        let source = raw.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(
            source.contains(
                "#![cfg_attr(not(debug_assertions), windows_subsystem = \"windows\")]"
            ),
            "src/main.rs 缺少 crate 级 `#![cfg_attr(not(debug_assertions), windows_subsystem = \"windows\")]`，\
             发布产物会多出一个空控制台窗口"
        );
    }

    /// 打包图标必须齐备 7 个原生层，且小于 256 的层不得用 PNG 压缩。
    ///
    /// `window_icon` 在运行期按显示器 DPI 从这份 `.ico` 里取原生层供给窗口图标；
    /// 而「按尺寸取层」的调用路径并非都认 PNG 压缩的小尺寸层 —— 资产脚本一旦退回全 PNG，
    /// 表现是某些调用方取不到对应尺寸而回落到别的尺寸或默认图标，编译与运行都不会报错。
    /// 故与上面两条同款做法：直接读产物断言结构。
    #[test]
    fn packaged_ico_keeps_native_layers_uncompressed() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/icons/icon.ico");
        let raw = std::fs::read(path).unwrap();
        let count = u16::from_le_bytes([raw[4], raw[5]]) as usize;

        let mut sizes = Vec::new();
        for i in 0..count {
            let entry = 6 + i * 16;
            let size = if raw[entry] == 0 {
                256
            } else {
                raw[entry] as usize
            };
            sizes.push(size);

            let len = u32::from_le_bytes(raw[entry + 8..entry + 12].try_into().unwrap()) as usize;
            let offset = u32::from_le_bytes(raw[entry + 12..entry + 16].try_into().unwrap()) as usize;
            let payload = &raw[offset..offset + len];

            if size < 256 {
                assert_eq!(
                    &payload[..4],
                    &[40u8, 0, 0, 0],
                    "{size}x{size} 层应为未压缩位图（BITMAPINFOHEADER），实际前 4 字节 {:?}；\
                     小尺寸层用 PNG 压缩会让部分取层路径取不到该尺寸",
                    &payload[..4]
                );
            } else {
                assert_eq!(
                    &payload[..8],
                    b"\x89PNG\r\n\x1a\n",
                    "256 层应保留 PNG 压缩"
                );
            }
        }

        sizes.sort_unstable();
        assert_eq!(
            sizes,
            vec![16, 24, 32, 48, 64, 128, 256],
            "打包图标须齐备 7 个原生尺寸层，实际 {sizes:?}"
        );
    }

    /// 发布档位必须保持体积导向：`lto = true`、`opt-level = "z"` 与 `codegen-units = 1` 都在，
    /// 且退栈未被关闭。
    ///
    /// 档位缺失（或被改回默认值）时，失效方式是**静默的**：编译、单测、出包全都不会失败，
    /// 只是产物体积悄悄回到 10.94 MB（实测）。反方向 `panic = "abort"` 会取消线程级 panic 隔离，
    /// 让 `download::run_blocking`「阻塞任务内的 panic 以 `Err` 回传」的既有契约静默失效
    /// （契约与对应用例只剩调试构建成立）。故与上面三条同款做法：直接读构建配置断言这几件事。
    #[test]
    fn release_profile_stays_size_oriented() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
        let raw = std::fs::read_to_string(path).unwrap();
        // ① 剔除注释行 —— 注释里会提到**被否决**的取值（`panic = "abort"` 等），它们不参与断言；
        // ② 去掉全部空白，使 `lto=true` 与 `lto = true`、以及换行与缩进都不影响判定。
        let active = raw
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .flat_map(|line| line.split_whitespace())
            .collect::<Vec<_>>()
            .join("");

        // 段内容取到下一个段头（`[`）为止；本段在文件末尾，故通常就是剩余全部。
        let profile = active
            .split("[profile.release]")
            .nth(1)
            .expect(
                "Cargo.toml 缺少 [profile.release] 段：发布构建会静默回落到 cargo 默认档位，\
                 产物体积回到 10.94 MB（实测）",
            )
            .split('[')
            .next()
            .unwrap();

        for needed in ["lto=true", "opt-level=\"z\"", "codegen-units=1"] {
            assert!(
                profile.contains(needed),
                "[profile.release] 缺少 `{needed}`：发布产物体积会显著回升\
                 （实测默认档位 10.94 MB、本档位 6.20 MB）"
            );
        }

        assert!(
            !profile.contains("panic="),
            "[profile.release] 不得关闭 panic 退栈：`panic = \"abort\"` 会让阻塞任务内的 panic \
             不再以 `Err` 回传（见 design D2）"
        );
    }
}
