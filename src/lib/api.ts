import { invoke } from '@tauri-apps/api/core';
import type {
  AppConfig,
  CompatStatus,
  ConnectRequest,
  ConnectResult,
  Device,
  DeviceStatusPayload,
  EnvStatus,
  InstallOutcome,
  ProxyList,
  SessionInfo,
  UpdateInfo,
} from './types';

// 与 src-tauri/src 中 `#[tauri::command]` 函数一一对应。

export const getConfig = (): Promise<AppConfig> => invoke('get_config');
export const saveConfig = (config: AppConfig): Promise<void> =>
  invoke('save_config', { config });

export const getCompatStatus = (): Promise<CompatStatus> =>
  invoke('get_compat_status');

export const getScrcpyDir = (): Promise<string> => invoke('get_scrcpy_dir');
export const openScrcpyDir = (): Promise<void> => invoke('open_scrcpy_dir');
/** 运行环境状态：目录 + 就绪判定 + 缺失清单（首启引导与「重新检测」共用）。 */
export const getEnvStatus = (): Promise<EnvStatus> => invoke('get_env_status');

export const getDevices = (): Promise<Device[]> => invoke('get_devices');

// ── 设备实时状态（Group 6 / tasks 6.3）──

/** 状态推送事件名（与后端 `device::EVENT_DEVICE_STATUS` 对齐）。 */
export const EVENT_DEVICE_STATUS = 'device-status';
/**
 * 状态快照。事件只在**变化时**推送，故启动时先取一次快照 ——
 * 否则首次推送若早于前端注册监听，界面会一直停在「检测中」。
 */
export const getDeviceStatus = (): Promise<DeviceStatusPayload> => invoke('get_device_status');
/**
 * 设备写命令（tasks 5.3）：返回**落盘后的完整配置**，前端整体替换自己的副本。
 * 只用设备列表做增量更新会漏掉后端连带改动的覆盖表（删除设备要清覆盖）。
 */
export const addDevice = (device: Device): Promise<AppConfig> =>
  invoke('add_device', { device });
export const updateDevice = (device: Device): Promise<AppConfig> =>
  invoke('update_device', { device });
export const deleteDevice = (id: string): Promise<AppConfig> =>
  invoke('delete_device', { id });

// ── 会话控制（Group 9 / tasks 9.1–9.4）──

/** 会话列表变化事件名（与后端 `session::EVENT_SESSION_CHANGED` 对齐）。 */
export const EVENT_SESSION_CHANGED = 'session-changed';

/**
 * 会话列表（含已结束的会话，供查看输出）。
 *
 * 事件只在会话变化时推送，故启动时先取一次 —— 与设备状态同理，避免「首次推送早于
 * 前端注册监听」让「日志」页一直显示空列表。
 */
export const listSessions = (): Promise<SessionInfo[]> => invoke('list_sessions');
/**
 * 连接单台设备；`args` 由前端按该设备自己的生效参数拼装。
 * 失败时抛出（原因直接用于提示用户），不静默降级。
 */
export const connectDevice = (request: ConnectRequest): Promise<ConnectResult> =>
  invoke('connect_device', { request });
/** 全部连接（tasks 9.2）：逐台回传结果；离线设备不会被尝试连接。 */
export const connectAll = (requests: ConnectRequest[]): Promise<ConnectResult> =>
  invoke('connect_all', { requests });
/** 断开单台设备，返回最新的会话列表。 */
export const disconnectDevice = (deviceId: string): Promise<SessionInfo[]> =>
  invoke('disconnect_device', { deviceId });
/** 全部断开（tasks 9.2），返回最新的会话列表。 */
export const disconnectAll = (): Promise<SessionInfo[]> => invoke('disconnect_all');
/** 某个会话的完整输出（tasks 9.4）。 */
export const getSessionLog = (deviceId: string): Promise<string> =>
  invoke('get_session_log', { deviceId });
/**
 * 清空某个会话已保留的输出。「日志」页的「清空」按钮走这里 —— 删的是后端那份缓冲，
 * 不是前端视图：只清视图的话 2 秒一轮的自动刷新会把旧行原样带回来。
 */
export const clearSessionLog = (deviceId: string): Promise<void> =>
  invoke('clear_session_log', { deviceId });

// ── 运行环境获取（Group 4 / tasks 4.1–4.9）──

export const getProxyList = (): Promise<ProxyList> => invoke('get_proxy_list');
/** 追加代理地址；格式非法或重复由后端拒绝并返回提示。 */
export const addProxyAddress = (url: string): Promise<ProxyList> =>
  invoke('add_proxy_address', { url });
/** 删除代理地址；列表内一律可删（含出厂默认的两条）。 */
export const removeProxyAddress = (url: string): Promise<ProxyList> =>
  invoke('remove_proxy_address', { url });

export const checkUpdate = (): Promise<UpdateInfo> => invoke('check_update');

/**
 * 开始获取运行环境。
 * - `bestFit`：首启缺失引导与「下载最佳适配版本」共用 —— 获取构建期锁定的受支持版本；
 * - `latest`：「下载最新版本」 —— 获取官方最新发布版本（查询失败时后端会回退到 bestFit）。
 */
export const startScrcpyInstall = (
  mode: 'bestFit' | 'latest',
): Promise<InstallOutcome> => invoke('start_scrcpy_install', { mode });
/** 确认「未经官方校验仍继续安装」（tasks 4.5）。 */
export const confirmScrcpyInstall = (): Promise<InstallOutcome> =>
  invoke('confirm_scrcpy_install');
/**
 * 中断当前通道的尝试、改用下一条（下载阶段可用）。
 *
 * 后端返回的 `Err` 只在「当前没有进行中的获取」时出现 —— 界面状态过期才会看到。
 */
export const switchScrcpyChannel = (): Promise<void> => invoke('switch_scrcpy_channel');
/**
 * 停止这次获取：中断在途下载/校验，并丢弃待确认的中间态。
 *
 * 两件事都幂等 —— 没有在途安装、没有待确认态时是空操作，因此「下载中的取消」与
 * 「未校验弹窗里的放弃」共用这一个命令。
 */
export const cancelScrcpyInstall = (): Promise<void> => invoke('cancel_scrcpy_install');

// ── 「关于」弹窗（about-page / tasks 5.1）──

/**
 * 用系统默认浏览器打开「关于」弹窗里的某个外链。
 *
 * 只传**语义键**、不传地址：地址表在 `src-tauri/src/links.rs`（全仓唯一来源），
 * 前端既拿不到地址、也无法请求打开任意 URL。未知键由后端拒绝，并回一句可直接展示的原因。
 */
export const openExternalLink = (
  link: 'website' | 'repo' | 'changelog' | 'support',
): Promise<void> => invoke('open_external_link', { link });

// ── 日志持久化（log-persistence / tasks 6.1）──

/**
 * 在系统文件管理器中定位日志目录（`%APPDATA%/FreeControl2/logs`，不存在时后端先创建）。
 *
 * 与 `openScrcpyDir` 同款；**没有**对应的「取日志目录路径」命令 —— 界面只给这一个入口，
 * 不展示目录路径与当前日志文件名（见 design D10）。
 */
export const openLogDir = (): Promise<void> => invoke('open_log_dir');

/**
 * 把主窗口显示到前台（app-shell 规格「用户断开最后一个设备后主窗口回到前台」）。
 *
 * 窗口调用（`show` / `unminimize` / `set_focus` / 置顶兜底）都在 Rust 侧这个命令里，
 * 故前端**不需要**任何额外的窗口权限声明；前台化结果由后端写入日志文件。
 */
export const bringMainWindowToFront = (): Promise<void> => invoke('bring_main_window_to_front');
