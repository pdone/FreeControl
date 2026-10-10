// 与后端 config.rs 对应的前端类型定义（camelCase，由 serde rename 对齐）。

export type DeviceKind = 'usb' | 'wifi';

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
 * 设备状态未知（尚未探测出结果，或该设备刚被加入还没轮询到）。
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
  error: string | null;
  /** 后端当前持有的设备列表（可能含轮询期间补全的 serial） */
  devices: Device[];
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
  error: string | null;
}

export interface ConnectResult {
  outcomes: ConnectOutcome[];
  sessions: SessionInfo[];
  /** 本次排列发生了槽位复用（空间不足） */
  degraded: boolean;
  /** 面向用户的提示（当前只有排列降级） */
  warning: string | null;
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

export interface AppConfig {
  schemaVersion: number;
  devices: Device[];
  defaults: Defaults;
  overrides: Record<string, OverrideEntry>;
  channels: Channels;
  logging: Logging;
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

/** 安装阶段（后端事件 `scrcpy-install-progress` 的 phase）。 */
export type InstallPhase = 'download' | 'verify' | 'extract' | 'replace' | 'done';

export interface InstallProgress {
  phase: InstallPhase;
  percent: number;
  text: string;
  /** 已生效（成功）的通道标签；尚未确定时为空串 */
  channel: string;
  /** 本次尝试的序号（从 1 开始）；0 表示该事件不带尝试信息 */
  attempt: number;
  /** 本次可用通道总数；0 表示该事件不带尝试信息 */
  attemptTotal: number;
  /** 正在尝试的通道标签；空串表示该事件不带尝试信息 */
  attemptChannel: string;
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
  /** 本次实际生效的下载通道标签 */
  channel: string;
  channelUrl: string;
  verified: boolean;
  unverifiedReason: string | null;
  note: string | null;
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
  /** 约定目录的完整路径（未就绪时同样返回，供手动放置） */
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
}
