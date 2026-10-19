import { writable, derived, get } from 'svelte/store';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { getVersion } from '@tauri-apps/api/app';
import { listen } from '@tauri-apps/api/event';
import { BUILTIN_DEFAULTS } from './defaults';
import { buildArgs, mergeParams } from './command';
import { validateParams, validateOverrideValues, type ParamErrors } from './validate';
import {
  getConfig,
  saveConfig,
  getCompatStatus,
  getDevices,
  getDeviceStatus,
  addDevice as addDeviceCmd,
  updateDevice as updateDeviceCmd,
  deleteDevice as deleteDeviceCmd,
  getEnvStatus,
  setScrcpyDir,
  getProxyList,
  addProxyAddress,
  removeProxyAddress,
  connectDevice as connectDeviceCmd,
  connectAll as connectAllCmd,
  disconnectDevice as disconnectDeviceCmd,
  disconnectAll as disconnectAllCmd,
  listSessions,
  getSessionLog,
  clearSessionLog as clearSessionLogCmd,
  bringMainWindowToFront as bringMainWindowToFrontCmd,
  EVENT_DEVICE_STATUS,
  EVENT_SESSION_CHANGED,
} from './api';
import type {
  AppConfig,
  CompatStatus,
  ConnectRequest,
  ConnectResult,
  Defaults,
  Device,
  DeviceStatus,
  DeviceStatusPayload,
  DeviceStatusView,
  EnvStatus,
  OverrideEntry,
  ProxyList,
  SessionInfo,
} from './types';

export const config = writable<AppConfig | null>(null);
export const devices = writable<Device[]>([]);
/**
 * 设备运行期状态表（design D13）：由后端统一判定后经事件下发，前端只渲染。
 * 设备列表与顶部指示读的是**同一份**数据，不各自计算。
 */
export const deviceStatus = writable<Record<string, DeviceStatus>>({});
/** 状态元信息：是否成功探测过、是否沿用上次结果（tasks 6.3）。 */
export const deviceStatusMeta = writable<{
  probed: boolean;
  stale: boolean;
  error: string | null;
}>({ probed: false, stale: false, error: null });
/**
 * 设备计数的**唯一来源**：只有顶栏的设备概况读它（设备页不再回显同一份计数，
 * 见 app-shell 规格「顶部状态指示」与 device-management 规格「设备列表汇总」），不各自 `filter`。
 *
 * - `total`：已登记设备数；
 * - `online`：在线数，**含已连接**（状态为 `connected` 或 `idle`），离线设备不计入；
 * - `connected`：已连接数，恒不大于 `online`。
 *
 * 尚未探测出状态的设备（状态表里没有该项）不计入任何一项 —— 界面把它们显示为「检测中…」。
 */
export const deviceCounts = derived(
  [devices, deviceStatus],
  ([$devices, $status]): { total: number; online: number; connected: number } => {
    let online = 0;
    let connected = 0;
    for (const d of $devices) {
      const s = $status[d.id];
      if (s === 'connected') {
        connected += 1;
        online += 1;
      } else if (s === 'idle') {
        online += 1;
      }
    }
    return { total: $devices.length, online, connected };
  },
);
/** 当前选中的设备（命令预览的目标，Group 8 使用）。 */
export const selectedDeviceId = writable<string | null>(null);
/**
 * 会话列表（Group 9）：含**已结束**的会话 —— 它们的输出保留下来供排查（tasks 9.4）。
 * 进程与槽位的事实在后端，前端只渲染。
 */
export const sessions = writable<SessionInfo[]>([]);
/** 「日志」页里当前查看的会话（设备 id）。 */
export const selectedSessionId = writable<string | null>(null);
/** 会话相关的一次性提示（批量连接结果、窗口排列降级等）。 */
export const sessionNotice = writable<string>('');
export const compat = writable<CompatStatus | null>(null);
/** scrcpy 运行环境状态（Group 3）：目录 · 就绪 · 缺失清单。 */
export const envStatus = writable<EnvStatus | null>(null);
/**
 * 运行环境目录。唯一来源是后端下发的 `envStatus.dir`，前端不自行拼路径，
 * 也就不会出现「界面上的路径 ≠ 实际检测的路径」。
 */
export const scrcpyDir = derived(envStatus, ($e): string => $e?.dir ?? '');
/** 首启缺失引导在本会话内是否已被用户关闭（不持久化：环境仍缺失时下次启动会再提示）。 */
export const envGuideDismissed = writable<boolean>(false);
/**
 * 下载通道列表：`direct` 是后端下发的只读首行（GitHub 直连，纯展示、不可点击），`proxies` 是
 * 配置文件里的代理地址（出厂默认 + 用户追加，一律可删）。前端不声明任何地址 —— 增删只看配置。
 */
export const proxies = writable<ProxyList>({ direct: '', proxies: [] });

/**
 * 同步代理列表并回写 config store。
 *
 * 后端增删代理时已直接落盘，这里必须同步更新 `config`，否则随后的去抖保存会用
 * 前端过期的 `channels.proxies` 覆盖掉后端的改动。
 */
function syncProxies(list: ProxyList): void {
  proxies.set(list);
  config.update((c) =>
    c ? { ...c, channels: { ...c.channels, proxies: list.proxies } } : c,
  );
}

export async function loadProxies(): Promise<void> {
  syncProxies(await getProxyList());
}

/**
 * 用后端返回的配置整体替换前端副本（tasks 5.3）。
 *
 * 设备写命令可能连带改动覆盖表（删除设备要清覆盖），所以必须整体替换而不是只更新 `devices`：
 * 否则随后的去抖保存会把前端残留的旧覆盖写回盘上，被删设备的覆盖又「复活」。
 */
function applyConfig(c: AppConfig): void {
  // 去抖窗口内可能已有未落盘的参数改动（默认参数 / 某台设备的覆盖取值）。设备写命令
  // 返回的是后端那份配置，直接整体替换会把这次改动吞掉，故按「保留本地」处理：
  //   · 默认参数：整体保留本地那份；
  //   · 覆盖表：只保留**新配置里仍然存在**的设备 —— 被删设备的覆盖必须以后端为准
  //     （后端已连带清除），留下就是「复活」。
  // 其余字段（设备、通道）一律以后端为准 —— 后端已落盘。
  const local = get(config);
  if (dirty && local) {
    const alive = new Set(c.devices.map((d) => d.id));
    const overrides: Record<string, OverrideEntry> = { ...c.overrides };
    for (const [id, entry] of Object.entries(local.overrides ?? {})) {
      if (alive.has(id)) overrides[id] = entry;
    }
    config.set({ ...c, defaults: local.defaults, overrides });
  } else {
    config.set(c);
  }
  devices.set(c.devices);
}

/** 两份设备列表是否等价（避免每 2 秒的状态推送都触发一次重渲染）。 */
function sameDevices(a: Device[], b: Device[]): boolean {
  if (a.length !== b.length) return false;
  return a.every((d, i) => JSON.stringify(d) === JSON.stringify(b[i]));
}

/**
 * 用后端下发的设备列表替换前端副本（tasks 6.1 / 6.3）。
 *
 * 只替换 `devices` 一侧，**不整体替换 `config`**：用户可能正有未落盘的默认参数改动
 * （去抖窗口内），整体替换会把它们丢掉。设备一侧始终以后端为准（后端已落盘）。
 */
function syncDevices(list: Device[]): void {
  if (sameDevices(get(devices), list)) return;
  devices.set(list);
  config.update((c) => (c ? { ...c, devices: list } : c));
}

/**
 * 应用一份设备状态快照（tasks 6.3）：状态、元信息与设备列表一起更新。
 *
 * 列表也一起同步，是因为轮询会在探测到 serial 后补全 USB 设备的连接标识 ——
 * 只更新状态会让界面上的标识停在「待检测」。
 */
export function applyDeviceStatus(payload: DeviceStatusPayload): void {
  deviceStatus.set(payload.statuses);
  deviceStatusMeta.set({
    probed: payload.probed,
    stale: payload.stale,
    error: payload.error,
  });
  syncDevices(payload.devices);
  // 已被删除的设备不能继续留在选中态（否则命令预览会指向一台不存在的设备）。
  selectedDeviceId.update((id) =>
    id && payload.devices.some((d) => d.id === id) ? id : null,
  );
}

/** 新增设备（tasks 5.3 / 6.1）：id 由后端分配，故新设备要从更新后的列表里认。 */
export async function addDevice(device: Device): Promise<Device | null> {
  const before = new Set(get(devices).map((d) => d.id));
  const c = await addDeviceCmd(device);
  applyConfig(c);
  const created = c.devices.find((d) => !before.has(d.id)) ?? null;
  if (created) selectedDeviceId.set(created.id);
  return created;
}

/** 更新设备（别名、连接标识）。 */
export async function updateDevice(device: Device): Promise<void> {
  applyConfig(await updateDeviceCmd(device));
}

/** 重命名（tasks 6.2）：别名内联编辑落盘 —— 只改这一个字段，不重发整份设备对象。 */
export async function renameDevice(id: string, alias: string): Promise<void> {
  const d = get(devices).find((x) => x.id === id);
  if (!d) return;
  await updateDevice({ ...d, alias });
}

/** 删除设备（tasks 6.5）：别名映射、参数覆盖与覆盖开关由后端一并清除。 */
export async function deleteDevice(id: string): Promise<void> {
  applyConfig(await deleteDeviceCmd(id));
  selectedDeviceId.update((sel) => (sel === id ? null : sel));
}

/** 追加代理地址；格式非法或重复时由后端拒绝，异常信息直接用于提示用户。 */
export async function addProxy(url: string): Promise<void> {
  syncProxies(await addProxyAddress(url));
}

/** 删除代理地址（列表内一律可删，含出厂默认的两条）。 */
export async function removeProxy(url: string): Promise<void> {
  syncProxies(await removeProxyAddress(url));
}

// ── 会话控制（Group 9 / tasks 9.1–9.4）──────────────────────────
//
// 分工：**参数在前端拼装**（8 组字段模型归前端所有，见 `params.ts` / `command.ts`），
// 后端只补 `-s <连接标识>` 与自动排列的 `--window-x` / `--window-y`（design D6）。
// 每台设备用**它自己的**生效参数，而不是当前选中设备的 —— 批量连接时每台设备的命令
// 必须各自成立（覆盖优先、非法值不参与拼装，与命令预览完全同源）。

/** 会话列表落地（事件与主动取快照共用）。 */
function applySessions(list: SessionInfo[]): void {
  sessions.set(list);
  // 选中的会话不存在了（记录被裁剪 / 设备被删除）就清空选择，避免日志面板指向不存在的会话。
  selectedSessionId.update((id) => (id && list.some((s) => s.deviceId === id) ? id : null));
}

/** 取一次会话列表（启动时用：事件只在变化时推，首推可能早于前端注册监听）。 */
export async function loadSessions(): Promise<void> {
  applySessions(await listSessions());
}

/**
 * 某台设备的生效参数：`enabled ? merge(默认参数, 覆盖取值) : 默认参数`（design D5）。
 *
 * 与 `effectiveParams` 同源同规则，只是把「哪台设备」作为显式参数 —— 连接时用的是
 * 该设备自己的参数，与当前选中哪台无关。
 */
function mergeForDevice(c: AppConfig | null, d: Defaults, deviceId: string | null): Defaults {
  const entry = deviceId ? c?.overrides?.[deviceId] : undefined;
  return mergeParams(
    d as unknown as Record<string, unknown>,
    isOverrideEnabled(entry) ? (entry?.values as Record<string, unknown>) : null,
  );
}

/** 任意设备的生效参数。 */
export function paramsForDevice(deviceId: string): Defaults {
  return mergeForDevice(get(config), get(defaults), deviceId);
}

/** 任意设备生效参数上的字段错误（拼装时据此剔除非法值，tasks 7.3）。 */
export function errorsForDevice(deviceId: string): ParamErrors {
  return validateParams(paramsForDevice(deviceId) as unknown as Record<string, Record<string, unknown>>);
}

/** 该设备本次要下发的参数（与命令预览同一条拼装路径）。 */
function argsForDevice(deviceId: string): string[] {
  return buildArgs(paramsForDevice(deviceId), errorsForDevice(deviceId));
}

/** 连接一台设备（tasks 9.1）；失败原因由调用方展示（不吞错）。 */
export async function connectDevice(deviceId: string): Promise<ConnectResult> {
  const request: ConnectRequest = { deviceId, args: argsForDevice(deviceId) };
  const res = await connectDeviceCmd(request);
  applySessions(res.sessions);
  sessionNotice.set(res.warning ?? '');
  return res;
}

/**
 * 一台设备在当前状态下是否可发起连接（设备卡上的「连接」入口据此判定）。
 *
 * 在线且未连接的设备直接连；**离线的无线设备**也可以连 —— 它的连接流程会先建立链路
 * （见 specs `device-management` 的「无线设备的链路建立」）；离线的 USB 设备不行，
 * 它的在线与否由物理接入决定，应用不对它做连接尝试。
 *
 * 「全部连接」不用这一判定：它只作用于在线且未连接的设备（见 [`connectAll`]）。
 */
export function canConnect(d: Device, status: DeviceStatusView | undefined): boolean {
  if (!d.connectionId) return false;
  return status === 'idle' || (status === 'offline' && d.kind === 'wifi');
}

/**
 * 连接入口被禁用时的原因文案；可连接时返回空串。
 *
 * 设备卡片上的「连接」与命令条上的连接按钮共用这一份 —— 两个入口不再各写一份措辞。
 * 判定以 [`canConnect`] 为准，其后按状态给原因，顺序与设备页原有 tooltip 一致
 * （connected → unknown → 连接标识待检测 → 离线 USB），保证两处逐字相同。
 */
export function connectBlockedReason(d: Device, status: DeviceStatusView | undefined): string {
  if (canConnect(d, status)) return '';
  if (status === 'connected') return '该设备已连接';
  if (status === 'unknown' || status === undefined) return '正在检测设备状态，请稍候';
  if (!d.connectionId) return '连接标识待检测（serial 未知），暂时无法连接';
  if (status === 'offline') return '设备离线：adb 中未检测到该设备，无法连接';
  return '';
}

/**
 * 全部连接（tasks 9.2）：只把**在线且未连接**的设备（状态值 `idle`，即界面上的「在线」）
 * 提交上去 —— 离线设备不提交，后端还会再拦一次（「离线设备不被尝试连接」这条不依赖前端筛选是否可靠）。
 */
export async function connectAll(): Promise<ConnectResult> {
  const statuses = get(deviceStatus);
  const requests: ConnectRequest[] = get(devices)
    .filter((d) => statuses[d.id] === 'idle' && !!d.connectionId)
    .map((d) => ({ deviceId: d.id, args: argsForDevice(d.id) }));
  if (requests.length === 0) {
    const notice = '没有可连接的设备（只有在线且未连接的设备会被连接，离线设备不会被尝试）';
    sessionNotice.set(notice);
    return { outcomes: [], sessions: get(sessions), degraded: false, warning: notice };
  }
  const res = await connectAllCmd(requests);
  applySessions(res.sessions);
  sessionNotice.set(res.warning ?? '');
  return res;
}

/** 断开一台设备（tasks 9.1）：会话结束、槽位释放，设备回到在线（未连接）。 */
export async function disconnectDevice(deviceId: string): Promise<void> {
  applySessions(await disconnectDeviceCmd(deviceId));
}

/** 全部断开（tasks 9.2）；返回结束后的会话列表。 */
export async function disconnectAll(): Promise<SessionInfo[]> {
  const list = await disconnectAllCmd();
  applySessions(list);
  return list;
}

/**
 * 把主窗口显示到前台（app-shell 规格「最后一个设备离开已连接状态后主窗口回到前台」）。
 *
 * **窗口调用全在 Rust 侧**（`main.rs` 的 `bring_main_window_to_front`）：先把窗口显示出来、
 * 从最小化恢复、再取前台；取不到焦点时（Windows 前台锁会静默拒绝，前端连失败都判不出来）
 * 由后端改用 **topmost 翻转** 把窗口抬到其它窗口之前。这样前端不需要任何窗口权限声明，
 * 而且前台化结果会写进日志文件 —— release 产物的 `console` 里看不到任何东西，日志是唯一的复核出口。
 *
 * 失败只向上抛一次：由 `watchLastDisconnect` 的 catch 兜住 —— 规格要求
 * 「前台化失败 MUST NOT 报错、MUST NOT 影响结束会话的结果」。
 */
async function bringMainWindowToFront(): Promise<void> {
  await bringMainWindowToFrontCmd();
}

/**
 * 监视「最后一个设备离开已连接状态」，据此把主窗口带回前台（app-shell 规格）。
 *
 * **为什么订阅状态而不是在各条界面路径上埋点**：会话可能因为用户点「断开」、在日志页结束、
 * **直接关掉投屏窗口（scrcpy 自行退出）**、异常退出、设备掉线、删除设备而结束 —— 埋点只能覆盖
 * 走前端动作的那几条，而「关掉投屏窗口」恰恰是最常见的用法。`deviceCounts` 是设备计数的
 * **唯一来源**（与顶栏设备概况、设备列表同源），它的跃迁正是规格要的「前后对比」。
 *
 * 两处必须守住：
 * - **基线**：以订阅那一刻的计数为基线（`last`），因此「启动时本来就是 0」不会被当成一次跃迁；
 * - **退出中不触发**：应用退出会结束全部会话、已连接数同样归零，但那一刻用户要的是退出
 *   （`closeHandled` 是关窗钩子已经接管的标志，见 `registerFlushOnClose`）。
 *
 * 前台化失败只落控制台：真正的复核出口是 Rust 侧写进日志的那一行。
 */
export function watchLastDisconnect(): void {
  let last = get(deviceCounts).connected;
  deviceCounts.subscribe((counts) => {
    const now = counts.connected;
    if (last > 0 && now === 0 && !closeHandled) {
      void bringMainWindowToFront().catch((e) => console.error('把主窗口带回前台失败', e));
    }
    last = now;
  });
}

/** 某个会话的输出（tasks 9.4）。 */
export function sessionLog(deviceId: string): Promise<string> {
  return getSessionLog(deviceId);
}

/**
 * 清空某个会话已保留的输出（「日志」页的「清空」）。
 *
 * 清的是后端那份缓冲而不是前端视图 —— 只清视图的话，2 秒一轮的自动刷新会把旧行原样带回来。
 * 会话记录本身（运行状态、退出码、实际命令）不受影响，其它会话的输出也不受牵连。
 */
export function clearSessionLog(deviceId: string): Promise<void> {
  return clearSessionLogCmd(deviceId);
}

/**
 * 当前页面 key。
 *
 * 注意**入口数与页键数不是同一个数**：侧边栏有 11 个入口（末尾是「关于」），而页键只有 10 个 ——
 * 「关于」不是页面，点它只开弹窗、不切页（见 app-shell 规格「统一导航入口」）。
 */
export const page = writable<string>('devices');
/** 侧边栏是否收起。 */
export const collapsed = writable<boolean>(false);

// ── 自绘标题栏（app-shell 规格「自定义标题栏」）──

/**
 * 主窗口是否带**系统装饰**（系统标题栏）。
 *
 * 判据取窗口的**事实**（`isDecorated()`），前端不另写一份平台清单：Windows 上后端在 `setup`
 * 里把装饰关掉（`main.rs::disable_system_decorations`），这里读到的就是 `false`；Linux 保持系统
 * 标题栏，读到 `true`。「要不要画那条自绘标题栏」与「系统画不画标题栏」因此是同一个事实的两面。
 *
 * 默认取 `true`（＝有系统装饰 → 不画自绘控件）：查询失败时停在与改动前一致的观感上，不会凭空
 * 多出三枚控件 —— 那种情况下窗口仍可用任务栏与 `Alt+F4` 关闭。
 */
export const windowDecorated = writable(true);

/** 主窗口当前是否最大化。最大化控件的图形（最大化 ↔ 向下还原）与它同源。 */
export const windowMaximized = writable(false);

/**
 * 系统是否已经在该窗口的**顶边**画了边框（Win11 画、Win10 不画）。
 *
 * 判据取窗口的**几何事实**：`outerPosition()` 是窗口外缘、`innerPosition()` 是客户区左上角，
 * 两者 y 之差就是「外缘到客户区上沿」那条非客户带的厚度 —— tao 交给系统的顶内缩正是它
 * （Win10 = 0，Win11 = 1 逻辑像素，见 tao 的 `calculate_insets_for_dpi`）。差值 > 0 ⇒ 系统
 * 已在顶边画了一条 ⇒ 界面不再补第二条（补了就成 Win11 上那条 2px 的粗顶边）。
 *
 * 与自绘标题栏同一套口径：取窗口事实，不写平台清单、不按版本号硬编码 —— 这个几何差量与 tao
 * 判的是**同一个量**，故不存在「两边判不一致」的可能。
 *
 * 默认取 `false`（＝系统没画 → 补画）：查询失败时停在与改动前一致的观感上。
 */
export const windowTopEdgeBySystem = writable(false);

/**
 * 初始化标题栏与上沿边线所需的窗口状态（挂载时调用一次）。
 *
 * 最大化状态**订阅窗口尺寸变化**，而不是只在本控件被点击时翻转：双击标题栏、系统快捷键、拖到
 * 屏幕边缘同样会改变最大化状态，只按点击历史推断会让图标与窗口对不上。最大化 / 还原必然伴随
 * 客户区尺寸变化，事件频率也低，不必节流。
 */
export async function initWindowChrome(): Promise<void> {
  const win = getCurrentWindow();
  try {
    windowDecorated.set(await win.isDecorated());
  } catch (e) {
    console.error('读取窗口装饰状态失败，将不呈现自绘标题栏', e);
  }
  // 顶边边线的判据用两次读数的**差值**，故窗口若在两次调用之间移动（读数整体错位）也无妨 ——
  // 相减时抵消，不必为此加锁。
  try {
    const outer = await win.outerPosition();
    const inner = await win.innerPosition();
    windowTopEdgeBySystem.set(inner.y - outer.y > 0);
  } catch (e) {
    console.error('读取窗口几何失败，上沿边线将按「系统未绘制顶边」处理（照旧补画）', e);
  }
  const syncMaximized = async (): Promise<void> => {
    try {
      windowMaximized.set(await win.isMaximized());
    } catch (e) {
      console.error('读取窗口最大化状态失败', e);
    }
  };
  await syncMaximized();
  try {
    await win.onResized(() => void syncMaximized());
  } catch (e) {
    console.error('订阅窗口尺寸变化失败，最大化状态将不再自动同步', e);
  }
}

/** 最小化主窗口（自绘标题栏的最小化键）。 */
export async function minimizeWindow(): Promise<void> {
  try {
    await getCurrentWindow().minimize();
  } catch (e) {
    console.error('最小化窗口失败', e);
  }
}

/** 在最大化与向下还原之间切换（自绘标题栏的最大化键）。 */
export async function toggleWindowMaximize(): Promise<void> {
  try {
    await getCurrentWindow().toggleMaximize();
  } catch (e) {
    console.error('切换窗口最大化失败', e);
  }
}

/**
 * 关闭主窗口（自绘标题栏的关闭键）。
 *
 * 走的就是系统关闭按钮那条路：`close()` 触发 `close-requested`，由 `registerFlushOnClose`
 * 拦下、先落盘再 `destroy()`。**不引入第二套退出逻辑，也不加二次确认**。
 */
export async function closeWindow(): Promise<void> {
  try {
    await getCurrentWindow().close();
  } catch (e) {
    console.error('关闭窗口失败', e);
  }
}

// ── 「关于」弹窗（about-page / tasks 5.2）──

/**
 * 「关于」弹窗是否打开。
 *
 * 放在 store 而不是弹窗组件内部：它的两个入口分处不同组件（侧边栏末位的「关于」与顶部的品牌行），
 * 需要一份共享开关；弹窗本身挂在应用级，与 `EnvGuide` / `UnverifiedConfirm` 同处。
 */
export const aboutOpen = writable<boolean>(false);

/**
 * 应用自身版本（「关于」弹窗的版本行之一）。
 *
 * 唯一来源是构建期定义并打包进程序的版本（Tauri 的 `getVersion()`，即 `tauri.conf.json` 的
 * `version`）—— 界面里 MUST NOT 再写版本字面量。`null` 表示尚未取到，弹窗那一行显示占位。
 */
export const appVersion = writable<string | null>(null);

/**
 * 生效的默认参数（8 组）：以内置默认值打底，再盖上盘上的那份。
 *
 * 打底而不是「盘上有就整份用它」，是为了让字段模型新增的键（以及被手工删掉的键）
 * 也有值 —— 否则拼装时会取到 `undefined`，表现为某个 flag 莫名其妙地漏发或误发。
 */
export const defaults = derived(config, ($c): Defaults =>
  mergeParams(
    BUILTIN_DEFAULTS as unknown as Record<string, unknown>,
    ($c?.defaults ?? {}) as Record<string, unknown>,
  ),
);

/** 默认参数的字段错误（8 组），供参数页就地提示（tasks 7.3）。 */
export const defaultsErrors = derived(defaults, ($d): ParamErrors =>
  validateParams($d as unknown as Record<string, Record<string, unknown>>),
);

// ── 设备级参数覆盖（tasks 7.4–7.8 / design D5）──────────────────────────
//
// 数据模型：`overrides[deviceId] = { enabled, values }`，`values` 形如
// `{ video: { maxFps: 30 } }`。`enabled=false` 时取值**原样保留**但不参与合并。

/** 覆盖表（deviceId → { enabled, values }）。 */
export const overrides = derived(
  config,
  ($c): Record<string, OverrideEntry> => $c?.overrides ?? {},
);

/**
 * 覆盖取值里的项数（组内按键计），用于参数面板标题的「覆盖已禁用（取值已保留）」判定。
 *
 * 该判据取「有取值」而非「有条目」：拨开开关后又关掉、一条值都没填的设备，不该在标题里
 * 报出一个空的禁用提示（用户看不到任何"已保留的取值"）。设备列表那枚标识已不再显示项数
 * （见 `isOverrideActive`）。
 */
export function countOverrideKeys(values: Record<string, unknown> | null | undefined): number {
  if (!values) return 0;
  let n = 0;
  for (const group of Object.values(values)) {
    if (group && typeof group === 'object' && !Array.isArray(group)) {
      n += Object.keys(group as Record<string, unknown>).length;
    }
  }
  return n;
}

/** 覆盖是否启用：只有显式 false 才算禁用（缺字段 / 新建条目均为启用）。 */
export function isOverrideEnabled(entry: OverrideEntry | undefined): boolean {
  return entry?.enabled !== false;
}

/**
 * 设备参数覆盖是否**已生效** —— 即设备列表那枚「已启用设备覆盖」标识的判据：
 * **至少有一项覆盖取值**，且**开关为启用**，两条缺一不可。
 *
 * MUST NOT 只调 `isOverrideEnabled`：它对 `undefined` 返回 true（缺字段即启用），而
 * `setOverrideEnabled` 拨一下开关就会写出 `{ enabled: true, values: {} }` —— 只调它会把
 * 这些"一条取值都没填"的设备也挂上标识。
 */
export function isOverrideActive(entry: OverrideEntry | undefined): boolean {
  return (
    isOverrideEnabled(entry) && countOverrideKeys(entry?.values as Record<string, unknown>) > 0
  );
}

/**
 * 写入设备覆盖的一项取值（tasks 7.8）。
 *
 * 新建条目时开关默认启用；写入后去抖落盘 —— 覆盖值与开关状态一并持久化（tasks 7.4）。
 */
export function setOverrideValue(
  deviceId: string,
  group: string,
  key: string,
  value: unknown,
): void {
  config.update((c) => {
    if (!c) return c;
    const prev = c.overrides?.[deviceId];
    const values = {
      ...((prev?.values as Record<string, Record<string, unknown>>) ?? {}),
    };
    values[group] = { ...(values[group] ?? {}), [key]: value };
    return {
      ...c,
      overrides: { ...c.overrides, [deviceId]: { enabled: isOverrideEnabled(prev), values } },
    };
  });
  scheduleSave();
}

/**
 * 覆盖启停开关（tasks 7.5 / 7.6）：禁用**不清值** —— 取值继续留在配置里，
 * 只是不再参与命令拼装；重新启用后立即恢复参与，设备列表的「已启用设备覆盖」标识同步恢复。
 */
export function setOverrideEnabled(deviceId: string, enabled: boolean): void {
  config.update((c) => {
    if (!c) return c;
    const prev = c.overrides?.[deviceId];
    return {
      ...c,
      overrides: {
        ...c.overrides,
        [deviceId]: { enabled, values: (prev?.values as Record<string, unknown>) ?? {} },
      },
    };
  });
  scheduleSave();
}

/** 各设备覆盖取值的字段错误（只校验覆盖中真正出现的键）。 */
export const overrideErrors = derived(config, ($c): Record<string, ParamErrors> => {
  const out: Record<string, ParamErrors> = {};
  for (const [id, entry] of Object.entries($c?.overrides ?? {})) {
    out[id] = validateOverrideValues(entry.values as Record<string, unknown>);
  }
  return out;
});

/**
 * 选中设备的生效参数：`enabled ? merge(默认参数, 覆盖取值) : 默认参数`（design D5）。
 * 命令预览与拼装都从这里取 —— 界面看到的与将要执行的是同一份。
 */
export const effectiveParams = derived(
  [defaults, config, selectedDeviceId],
  ([$d, $c, $id]): Defaults => mergeForDevice($c, $d, $id),
);

/** 生效参数上的字段错误：命令拼装据此剔除非法值（tasks 7.3）。 */
export const effectiveErrors = derived(effectiveParams, ($p): ParamErrors =>
  validateParams($p as unknown as Record<string, Record<string, unknown>>),
);

/**
 * 去抖窗口（毫秒）。短于它的连续修改只落盘一次 —— 拖动滑块、连点开关时
 * 每次击键都写盘既无必要也会放大「写一半被打断」的机会。
 */
const SAVE_DEBOUNCE_MS = 400;

let saveTimer: ReturnType<typeof setTimeout> | null = null;
/** 是否有改动尚未落盘（仍处在去抖窗口里也算）。 */
let dirty = false;

/** 去抖保存配置（tasks 5.2：去抖在这里，原子写由后端保证）。 */
export function scheduleSave(): void {
  dirty = true;
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveTimer = null;
    void flushSave();
  }, SAVE_DEBOUNCE_MS);
}

/**
 * 立即把未保存的改动落盘，并取消排队中的去抖任务（tasks 5.2 / 5.4）。
 *
 * 无改动时不做任何 IO。退出前必须走一次：最后一次修改可能还卡在去抖窗口里，
 * 直接关窗会让「改完就退出」的那次修改丢掉 —— 与「修改后重启应用仍生效」直接相关。
 */
export async function flushSave(): Promise<void> {
  if (saveTimer) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
  if (!dirty) return;
  dirty = false;
  const c = get(config);
  if (!c) return;
  try {
    await saveConfig(c);
  } catch (e) {
    dirty = true; // 下次仍有机会写出去
    console.error('保存配置失败', e);
  }
}

/** 是否已经处理过一次关窗（见下方：防止 destroy 失败后窗口被锁死）。 */
let closeHandled = false;

/**
 * 关窗前刷一次未落盘的改动（tasks 5.2）。
 *
 * 先拦下关窗动作，落盘完成后再真正销毁窗口；落盘失败或超时都照常关闭 ——
 * 卡住窗口比丢一次改动更糟。
 *
 * **依赖能力声明**：`destroy` / `close` 都不在 `core:default` 里，必须由
 * `src-tauri/capabilities/default.json` 显式授予 `core:window:allow-destroy`。
 * 少了它，`event.preventDefault()` 生效而后续的销毁被拒绝 —— 表现为
 * 「点标题栏关闭按钮没反应、程序也不退出」（曾发生）。改动这份能力声明前请先读这里。
 */
async function registerFlushOnClose(): Promise<void> {
  const win = getCurrentWindow();
  await win.onCloseRequested(async (event) => {
    // 已经拦过一次就不再拦：万一销毁失败（权限缺失 / IPC 异常），第二次点关闭
    // 必须能走原生路径退出，不能把窗口锁死。
    if (closeHandled) return;
    closeHandled = true;
    event.preventDefault();
    await Promise.race([
      flushSave(),
      new Promise((resolve) => setTimeout(resolve, 800)),
    ]);
    try {
      await win.destroy();
    } catch (e) {
      // 退一步请求按原生方式关闭：上面已置 closeHandled，这次不会再被本回调拦住。
      console.error('销毁窗口失败，改为请求原生关闭', e);
      await win.close().catch((err) => console.error('关闭窗口失败', err));
    }
  });
}

/**
 * 启动期放主窗口出来。
 *
 * 主窗口以**隐藏方式**创建（`src-tauri/tauri.conf.json` 的 `visible: false`）：窗口先出现、
 * 界面后渲染的那段空档里，屏幕上只有一块 `--bg` 纯色，而这段空档**无法用 HTML/CSS 填充**
 * （实测：webview 的首个可绘制帧出现在外壳渲染完成之后，页面里放什么标记都赶不上那一帧）。
 * 于是反过来 —— 等外壳渲染完，再把窗口放出来。
 *
 * **为什么由前端发这个信号**：Svelte 4 的 `new App(...)` 是**同步渲染**，它返回时外壳已在
 * DOM 中，这是「界面已就绪」最直接的定义。它 **MUST NOT** 改成"等数据加载完" ——
 * 窗口只等外壳，不等数据。放到屏幕上的时机另由 `revealWindowWhenPainted` 把关。
 *
 * 一个曾经担心、后来被实测推翻的点：窗口隐藏期间，WebView2 里的页面**仍被判为可见**，
 * `requestAnimationFrame` 照常触发 —— 所以"等首帧真的画出来"这件事在隐藏状态下也做得到
 * （`revealWindowWhenPainted` 正是这么做的）。
 *
 * **依赖能力声明**：`show` **不在** `core:default` 里（该默认集只有窗口的只读命令），必须由
 * `src-tauri/capabilities/default.json` 显式授予 `core:window:allow-show`。少了它的表现是
 * 「每次启动都静默多等一次兜底超时」，界面本身看不出任何异常
 * （`src-tauri/src/main.rs` 有对应的不变量单测）。改动这份能力声明前请先读这里。
 *
 * 失败只记录：Rust 侧另有两条兜底期限，窗口不会因此不出现。
 */
export async function revealWindow(): Promise<void> {
  try {
    await getCurrentWindow().show();
  } catch (e) {
    console.error('显示窗口失败（将由兜底期限显示）', e);
  }
}

/**
 * 等界面**首帧真的画出来**之后再放窗口（启动期的调用点，见 `main.ts`）。
 *
 * 外壳进入 DOM（Svelte 同步渲染完）并不等于它已经亮出来：浏览器还要布局、绘制、提交那一帧。
 * 这中间可能跨好几个帧 —— 实测 release 下"进入 DOM → 首次内容绘制"约 99ms（主线程被外壳的
 * 布局与图标渲染占着）。所以**不能靠"等 N 帧"**：等两帧后实测窗口仍比首帧早约 52ms 上屏，
 * 那 3 帧里用户看到的就是纯色。这里改等**首次内容绘制（FCP）上报**：该条目一上报，
 * 含外壳的那一帧就已经提交，窗口亮出来就是界面本身。
 *
 * 兜底：FCP 若永远不来（平台异常）也不会把窗口锁死 —— Rust 侧另有两条期限（见 `main.rs`
 * 的 `spawn_reveal_fallback`），到点会无条件把窗口显示出来。
 */
export function revealWindowWhenPainted(): void {
  const painted = (entries: PerformanceEntryList): boolean =>
    entries.some((e) => e.name === 'first-contentful-paint');
  // `buffered: true`：FCP 若已经发生过，订阅时就会立刻收到那条历史条目，不会漏。
  const observer = new PerformanceObserver((list) => {
    if (painted(list.getEntries())) {
      observer.disconnect();
      void revealWindow();
    }
  });
  observer.observe({ type: 'paint', buffered: true });
}

/**
 * 重新检测运行环境（tasks 3.4）：兼容性状态与就绪状态取自**同一次刷新**，
 * 保证顶部指示、环境卡片与命令预览读到的是同一份结果（design D13 同一份状态数据的要求）。
 * 失败时抛出，由调用方决定是静默记录（启动阶段）还是给出可见反馈（手动「重新检测」）。
 */
export async function refreshEnv(): Promise<void> {
  const [c, env] = await Promise.all([getCompatStatus(), getEnvStatus()]);
  compat.set(c);
  envStatus.set(env);
}

/**
 * 设置 / 清除自定义运行环境目录。
 *
 * 后端返回落盘后的完整配置：必须用它替换前端副本，否则下一次去抖保存会把旧目录写回盘上。
 * 随后刷新运行环境状态，让路径、就绪判定与版本探测同步到新目录。
 */
export async function updateScrcpyDir(path: string | null): Promise<void> {
  applyConfig(await setScrcpyDir(path));
  await refreshEnv();
}

/** 应用启动后拉取外壳所需的全部数据。 */
export async function initApp(): Promise<void> {
  // 关窗前刷未落盘改动的钩子（tasks 5.2）；注册失败只记录，不影响其它初始化。
  registerFlushOnClose().catch((e) => console.error('注册关窗保存钩子失败', e));
  // 标题栏所需的两处窗口状态（app-shell 规格「自定义标题栏」）：不等它 —— 外壳先上屏，
  // 查询通常在首帧绘制之前就回来（本地 IPC），因此不会出现控件后到的跳动。
  void initWindowChrome();
  try {
    const c = await getConfig();
    // 默认参数缺失（Null）时回落内置并回写（tasks 5.1 / 5.4：
    // 首次启动结束时盘上就是完整的默认参数，而不是一个空值）。
    if (!c.defaults || Object.keys(c.defaults).length === 0) {
      c.defaults = BUILTIN_DEFAULTS;
      await saveConfig(c);
    }
    config.set(c);
  } catch (e) {
    console.error('加载配置失败', e);
    config.set({
      schemaVersion: 1,
      scrcpyDir: null,
      devices: [],
      defaults: BUILTIN_DEFAULTS,
      overrides: {},
      channels: { proxies: [] },
      // 读配置都失败时用的兜底快照：保留策略取出厂默认（`config::RETENTION_7D`），
      // 与后端「缺字段即 7 天」一致。
      logging: { retention: '7d' },
    });
  }
  try {
    devices.set(await getDevices());
  } catch (e) {
    console.error('加载设备失败', e);
  }
  try {
    // 先订阅再取快照：后端只在状态**变化**时推送，若只订阅不取快照，
    // 首次推送早于本次订阅（启动竞态）就会让界面一直停在「检测中」。
    await listen<DeviceStatusPayload>(EVENT_DEVICE_STATUS, (e) =>
      applyDeviceStatus(e.payload),
    );
    applyDeviceStatus(await getDeviceStatus());
  } catch (e) {
    console.error('加载设备状态失败', e);
  }
  // 「最后一个设备离开已连接状态」的监视（app-shell 规格）：必须在上面那份**首帧快照之后**
  // 订阅 —— 订阅那一刻的计数即基线，否则启动时本来就是 0 的情形会被当成一次跃迁。
  watchLastDisconnect();
  try {
    // 先订阅再取列表（与设备状态同理：后端只在会话变化时推送）。
    await listen<SessionInfo[]>(EVENT_SESSION_CHANGED, (e) => applySessions(e.payload));
    await loadSessions();
  } catch (e) {
    console.error('加载会话列表失败', e);
  }
  try {
    await loadProxies();
  } catch (e) {
    console.error('加载代理地址列表失败', e);
  }
  try {
    // 应用版本（「关于」弹窗的版本行）：取 Tauri 打包时用的那个版本。
    // 失败只记录 —— 弹窗那一行停在占位，启动不受影响（与上面几处同法）。
    appVersion.set(await getVersion());
  } catch (e) {
    console.error('读取应用版本失败', e);
  }
  try {
    await refreshEnv();
  } catch (e) {
    console.error('检测运行环境失败', e);
  }
}
