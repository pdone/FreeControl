// 与后端 config.rs 对应的前端类型定义（camelCase，由 serde rename 对齐）。

export type DeviceKind = 'usb' | 'wifi';

/**
 * 后端上屏消息：稳定 code + 可选参数。
 * 文案由前端字典按当前语言出；`detail` 只承载技术细节，原样展示。
 */
export interface UserMessage {
  code: string;
  params?: Record<string, string | number>;
}

export interface UserError extends UserMessage {
  detail?: string;
}

/** 下载通道的展示形状：官方通道没有地址，代理通道带自己的前缀地址。 */
export type ChannelView = { kind: 'official' } | { kind: 'proxy'; url: string };

export interface Device {
  id: string;
  alias: string;
  kind: DeviceKind;
  serial: string;
  ip: string;
  port: number;
  /**
   * 连接标识（`-s` 参数用的值）：USB 为 serial，无线为 `IP:端口`。
   * 由后端计算后下发（后端只读派生字段），前端不自己拼 —— 算法只有一份。
   * 新增设备时不填（后端会忽略）。
   */
  connectionId?: string;
}

/**
 * 运行期状态：由后端统一判定后下发，前端只渲染（design D13）。
 *
 * 取值是**前后端契约**，不随界面措辞改名：`idle` 就是界面上的「在线」
 * （已探测到、可发起连接、当前未连接）；`connected` 是「已连接」，`offline` 是「离线」。
 */
export type DeviceStatus = 'connected' | 'idle' | 'offline';

/**
 * 设备状态未知（尚未探测出结果，或该设备刚被加入还没等到订阅通知）。
 * 界面显示「检测中…」：既不计入「在线」也不计入「已连接」。
 */
export type DeviceStatusView = DeviceStatus | 'unknown';

/** 设备状态快照（tasks 6.3）：事件推送与 `get_device_status` 命令共用。 */
export interface DeviceStatusPayload {
  /** 设备 id → 状态 */
  statuses: Record<string, DeviceStatus>;
  /** 最近一次探测失败、当前结果沿用上一次成功探测 */
  stale: boolean;
  /** 是否至少成功探测过一次（未成功时前端显示「检测中」而不是「离线」） */
  probed: boolean;
  /** 最近一次探测失败的原因（成功时为 null） */
  error: UserError | null;
  /** 后端当前持有的设备列表（可能含订阅期间补全的 serial） */
  devices: Device[];
}

// ── 无线配对与发现（spec `device-management` 的三条新增要求）──

/**
 * 扫描结果里的一台设备。
 *
 * `registered` 按 **IP** 判定：同一台设备的连接端口会随设备侧重启而变化，
 * 连接标识不能当身份用（否则会出现「明明是同一台，却每条端口都算新设备」）。
 */
export interface DiscoveredDevice {
  ip: string;
  port: number;
  /** 连接标识（`IP:端口`），与 `Device.connectionId` 同形 */
  connectionId: string;
  registered: boolean;
  /** 来源：多播发现 / 5555 网段扫描 */
  source: 'mdns' | 'sweep';
}

/**
 * 一次发现窗的结果。
 *
 * 多播本身不可用时后端仍返回 `devices: []` + `mdnsError`：界面据此决定要不要露出
 * 5555 兜底入口，而不是把整次扫描判成失败。
 */
export interface WirelessScan {
  devices: DiscoveredDevice[];
  mdnsError: UserError | null;
}

/** 一台等待配对的设备（配对服务只在设备弹窗打开期间存在，故每次都要现查）。 */
export interface PairingTarget {
  ip: string;
  port: number;
  connectionId: string;
}

/** 配对请求：`address` 是设备的配对地址（`IP:配对端口`）。 */
export interface PairRequest {
  address: string;
  code: string;
  /** 用户指定的别名；留空则由机型推导 */
  alias?: string;
}

/** 加入请求：把扫描结果里的一台设备连上并登记。 */
export interface JoinRequest {
  ip: string;
  port: number;
  alias?: string;
}

/** 配对 / 加入成功后的结果。 */
export interface WirelessJoin {
  deviceId: string;
  alias: string;
  connectionId: string;
}

/**
 * 一个会话（Group 9）：一台设备一个受本应用管理的 scrcpy 进程。
 *
 * 进程退出后记录**不会被丢弃** —— 异常退出时的输出正是排查依据（tasks 9.4），
 * 因此 `running=false` 的会话同样出现在列表里，可继续查看其日志。
 */
export interface SessionInfo {
  deviceId: string;
  alias: string;
  connectionId: string;
  pid: number;
  /** 启动时刻（Unix 毫秒） */
  startedAt: number;
  /** 实际执行的命令（形如 `scrcpy -s <标识> <参数…>`，与命令预览同构） */
  command: string;
  /** 是否参与自动窗口排列（用户显式指定 `--window-x` / `--window-y` 时为 false） */
  arranged: boolean;
  /**
   * 未参与自动排列的原因（design D5）：`user` = 用户显式指定了位置；
   * `platform` = 显示协议不允许应用定位窗口（如 Wayland 会话），窗口位置由系统决定。
   * 参与排列时为 null —— 两种原因的提示文案完全不同，前端据此分支。
   */
  arrangeSkip: 'user' | 'platform' | null;
  slot: number | null;
  windowX: number | null;
  windowY: number | null;
  /** 空间不足而复用了已占用的槽位（窗口可能与其他窗口重叠，tasks 9.8） */
  slotReused: boolean;
  running: boolean;
  exitCode: number | null;
  logLines: number;
}

/** 连接请求：参数由前端拼装（字段模型归前端所有），后端补 `-s` 与自动排列的位置。 */
export interface ConnectRequest {
  deviceId: string;
  args: string[];
}

/** 单台设备的连接结果（批量时逐台回传，便于把失败原因贴到对应设备上）。 */
export interface ConnectOutcome {
  deviceId: string;
  alias: string;
  ok: boolean;
  error: UserError | null;
}

export interface ConnectResult {
  outcomes: ConnectOutcome[];
  sessions: SessionInfo[];
  /** 本次排列发生了槽位复用（空间不足） */
  degraded: boolean;
  /** 面向用户的提示（当前只有排列降级） */
  warning: UserMessage | null;
}

export interface OverrideEntry {
  enabled: boolean;
  values: Record<string, unknown>;
}

export interface Channels {
  /** 代理地址列表：出厂默认 + 用户追加，顺序即自动降级的顺序；列表内一律可删。 */
  proxies: string[];
}

/**
 * 日志设置（后端 `config::Logging`）：目前只有保留策略。
 *
 * 取值是**前后端契约**：`7d` / `30d` / `forever`。后端把未知或缺失的取值一律按出厂默认
 * （`7d`）处理，因此前端不必为「读到没见过的值」准备分支。
 */
export interface Logging {
  retention: string;
}

/** 生效语言：中英两套字典。 */
export type Lang = 'zh' | 'en';

/** 界面语言偏好：跟随系统或固定一档。 */
export type LanguagePref = 'system' | Lang;

/**
 * 本机字体（后端 `fonts::SystemFont`）。
 *
 * `family` 是落盘与 CSS 使用的 canonical 族名；`aliases` 保留全部语言的族名，
 * 供中文界面显示与两种名字的搜索；`monospaced` 只用于列表上的等宽标记。
 */
export interface SystemFont {
  family: string;
  aliases: string[];
  monospaced: boolean;
}

/**
 * 外观偏好（后端 `config::Ui`）：界面语言、主题模式、主题色与两个字体字段。
 *
 * - `language`：`system`（跟随系统，出厂默认）/ `zh` / `en`；
 * - `theme`：`system`（跟随系统，出厂默认）/ `dark` / `light`；
 * - `accent`：`#rrggbb`（小写；预设值与自定义 RGB 归一化后的结果同形）；
 * - `uiFont` / `codeFont`：界面字体与代码字体的族名；`null` 表示系统默认。
 *
 * 后端把未知语言、未知主题、非法主题色与非字符串字体字段一律退化为出厂默认，
 * 因此前端不必为「读到没见过的值」准备分支。这些字段**不属于** `defaults.*`
 * （scrcpy 参数），改动不经过参数组。
 */
export interface UiPrefs {
  language: LanguagePref;
  theme: 'system' | 'dark' | 'light';
  accent: string;
  uiFont: string | null;
  codeFont: string | null;
}

/**
 * 更新检查偏好（后端 `config::Update`）：自动检查开关、频率与上次检查时间。
 *
 * 频率是前后端契约：`launch` / `daily` / `weekly` / `monthly`；后端把未知取值退为
 * `launch`，前端不必为未知取值准备分支。`lastCheckedAt` 是 Unix 毫秒，`null` 表示从未检查；
 * 手动与自动、成功与失败都写同一个时间。
 */
export type UpdateFrequency = 'launch' | 'daily' | 'weekly' | 'monthly';

export interface UpdatePrefs {
  autoCheck: boolean;
  frequency: UpdateFrequency;
  lastCheckedAt: number | null;
}

export interface AppConfig {
  schemaVersion: number;
  /** 自定义 scrcpy 运行环境目录（绝对路径）；`null` 表示使用约定目录。 */
  scrcpyDir: string | null;
  devices: Device[];
  defaults: Defaults;
  overrides: Record<string, OverrideEntry>;
  channels: Channels;
  logging: Logging;
  ui: UiPrefs;
  update: UpdatePrefs;
}

/** 8 组默认参数（前端拥有字段模型；后端透明存储为 JSON）。 */
export interface Defaults {
  video: Record<string, unknown>;
  source: Record<string, unknown>;
  audio: Record<string, unknown>;
  control: Record<string, unknown>;
  record: Record<string, unknown>;
  window: Record<string, unknown>;
  apps: Record<string, unknown>;
  advanced: Record<string, unknown>;
}

/**
 * 下载通道列表。
 *
 * `direct` 是首行的 GitHub 直连地址：只读、纯展示（不可点击、无跳转），**不在** `proxies` 里 ——
 * 直连不是代理、也没有可管理性，混进列表只会逼前端再加一个「是否只读」的标志。
 */
export interface ProxyList {
  direct: string;
  proxies: string[];
}

/** 检查更新结果（tasks 4.9）。 */
export interface UpdateInfo {
  latest: string;
  bestFit: string;
  installed: string;
  state: 'upToDate' | 'updateAvailable' | 'installedNewer' | 'notInstalled';
}

// ── 应用自身更新（spec `app-update`）──

/** 检查结果：只给出界面需要的状态键与两个版本号。 */
export interface AppUpdateCheck {
  current: string;
  latest: string;
  state: 'upToDate' | 'updateAvailable' | 'installedNewer' | 'ready';
}

/** 应用更新进度事件（`app-update-progress`）。 */
export interface AppUpdateProgress {
  phase: 'download' | 'verify';
  percent: number;
  channelView: ChannelView | null;
}

export interface AppUpdateOutcome {
  status: 'ready' | 'canceled';
  version: string;
}

/** 命令失败的类型（界面据此选文案，`message` 只用于日志）。 */
export interface AppUpdateError {
  kind: 'check' | 'download' | 'noAsset' | 'write';
  message: string;
}

/** 安装阶段（后端事件 `scrcpy-install-progress` 的 phase）。 */
export type InstallPhase = 'download' | 'verify' | 'extract' | 'replace' | 'done';

export interface InstallProgress {
  phase: InstallPhase;
  percent: number;
  /** 本次尝试的序号（从 1 开始）；0 表示该事件不带尝试信息 */
  attempt: number;
  /** 本次可用通道总数；0 表示该事件不带尝试信息 */
  attemptTotal: number;
  /** 当前资产文件名 */
  asset: string | null;
  /** 已下载 / 已解压的字节数或条目数 */
  done: number | null;
  /** 总字节数或总条目数 */
  total: number | null;
  /** 安装完成时的版本号 */
  version: string | null;
  /** 已生效通道的结构化形状 */
  channelView: ChannelView | null;
  /** 正在尝试通道的结构化形状 */
  attemptChannelView: ChannelView | null;
  /** 替换阶段是否先结束了 adb 服务端 */
  replaceStoppingAdb: boolean | null;
}

/**
 * 一次安装的结果。
 *
 * - `done`：已完成；
 * - `needsConfirmation`：下载未经官方校验、等待用户确认（tasks 4.5）；
 * - `canceled`：用户在下载/校验阶段点了「取消」。**不是错误** —— 不写 `installError`。
 */
export interface InstallOutcome {
  status: 'done' | 'needsConfirmation' | 'canceled';
  version: string;
  asset: string;
  channelUrl: string;
  verified: boolean;
  /** 已生效通道的结构化形状；取消时为空 */
  channelView: ChannelView | null;
  /** 未经官方校验的原因（结构化） */
  unverifiedReasonMessage: UserError | null;
  /** 下载目标回退等说明（结构化） */
  noteMessage: UserMessage | null;
}

export interface CompatStatus {
  bestFit: string;
  installed: string;
  status: 'ok' | 'mismatch';
  direction: 'equal' | 'older' | 'newer' | 'unknown';
}

/**
 * scrcpy 运行环境状态（Group 3）。
 * 就绪判定完全在后端完成，前端只渲染 `ready` / `missing`，不自行检查文件。
 */
export interface EnvStatus {
  /** 生效目录的完整路径（自定义优先，否则为约定目录；未就绪时同样返回，供手动放置） */
  dir: string;
  /**
   * 界面「约定目录」的说明文案：后端按平台拼好下发（Windows 为
   * `%APPDATA%/FreeControl2/scrcpy-{OS}-v{Version}`），前端逐字渲染，MUST NOT 自拼平台前缀。
   */
  dirHint: string;
  /** 目录内必需可执行文件（scrcpy 与 adb）是否齐备 */
  ready: boolean;
  /** 缺失的必需可执行文件名 */
  missing: string[];
  /** 当前是否使用用户自定义目录 */
  isCustom: boolean;
}
