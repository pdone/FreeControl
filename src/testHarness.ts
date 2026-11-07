// 仅在浏览器测试入口使用：以本地状态替身驱动真实 DevicePage。
// 不进入 index.html / 生产构建（Vite 只以 index.html 为入口）。
import './app.css';
import { get } from 'svelte/store';
import DevicePage from './components/DevicePage.svelte';
import TooltipLayer from './components/TooltipLayer.svelte';
import { BUILTIN_DEFAULTS } from './lib/defaults';
import { applyAppearance } from './lib/theme';
import { applyLanguage } from './lib/i18n';
import {
  applyDeviceStatus,
  applyDeviceListSnapshot,
  config as configStore,
  devices,
  sortedDevices,
  page,
  selectedDeviceId,
  sessions,
} from './lib/store';
import type {
  AppConfig,
  Device,
  DeviceListPrefs,
  DeviceStatus,
  SessionInfo,
} from './lib/types';

// 相对时间用距当前时刻的偏移，覆盖分钟 / 小时 / 天三档；排序仍按这些值的先后。
const NOW = Date.now();
const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

const baseDevices: Device[] = [
  { id: 'dev-1', alias: '客厅平板', kind: 'wifi', serial: '', ip: '192.168.1.5', port: 5555, connectionId: '192.168.1.5:5555', lastSessionStartedAt: NOW - 2 * MINUTE },
  { id: 'dev-2', alias: '主力机', kind: 'usb', serial: 'SERIAL-2', ip: '', port: 0, connectionId: 'SERIAL-2', lastSessionStartedAt: NOW - 30_000 },
  { id: 'dev-3', alias: '备用机', kind: 'usb', serial: 'SERIAL-3', ip: '', port: 0, connectionId: 'SERIAL-3', lastSessionStartedAt: null },
  { id: 'dev-4', alias: '设备 10', kind: 'usb', serial: 'SERIAL-4', ip: '', port: 0, connectionId: 'SERIAL-4', lastSessionStartedAt: NOW - 3 * HOUR },
  { id: 'dev-5', alias: '设备 2', kind: 'wifi', serial: '', ip: '192.168.1.8', port: 5555, connectionId: '192.168.1.8:5555', lastSessionStartedAt: null },
  { id: 'dev-6', alias: 'Studio', kind: 'usb', serial: 'SERIAL-6', ip: '', port: 0, connectionId: 'SERIAL-6', lastSessionStartedAt: NOW - 3 * DAY },
];

let statuses: Record<string, DeviceStatus> = {
  'dev-1': 'idle',
  'dev-2': 'connected',
  'dev-3': 'offline',
  'dev-4': 'idle',
  'dev-5': 'offline',
  'dev-6': 'offline',
};

const sessionFor = (d: Device): SessionInfo => ({
  deviceId: d.id,
  alias: d.alias,
  connectionId: d.connectionId ?? '',
  pid: 4242,
  startedAt: d.lastSessionStartedAt ?? 0,
  command: `scrcpy -s ${d.connectionId ?? ''}`,
  arranged: true,
  arrangeSkip: null,
  slot: 0,
  windowX: 8,
  windowY: 8,
  slotReused: false,
  running: true,
  exitCode: null,
  logLines: 0,
});

let config: AppConfig = {
  schemaVersion: 1,
  scrcpyDir: null,
  devices: [...baseDevices],
  deviceList: {
    sortMode: 'manual',
    directions: { status: 'asc', name: 'asc', lastConnected: 'desc' },
    manualOrder: baseDevices.map((d) => d.id),
    revision: 1,
  },
  defaults: BUILTIN_DEFAULTS,
  overrides: {},
  channels: { proxies: [] },
  logging: { retention: '7d' },
  ui: {
    language: 'zh',
    theme: 'dark',
    accent: '#6366f1',
    uiScale: '100',
    deviceListDensity: 'detailed',
    uiFont: null,
    codeFont: null,
  },
  update: { autoCheck: false, frequency: 'launch', lastCheckedAt: null },
};

let statusSequence = 1;
let failReorder = false;
let failMode = false;
/** 下一次连接抛出的错误；用于验证行内问题指示的长原文与提示层。 */
let failConnect: unknown = null;
let failDirection = false;

const snapshot = () => structuredClone({
  devices: config.devices,
  deviceList: config.deviceList,
});

const bump = (): void => {
  config.deviceList.revision += 1;
};

const updateStatus = (id: string, status: DeviceStatus): void => {
  statuses[id] = status;
};

/** 模拟后端推送一份新的运行态快照（序号递增）。 */
const pushStatuses = (): void => {
  statusSequence += 1;
  applyDeviceStatus({
    statuses: { ...statuses },
    stale: false,
    probed: true,
    error: null,
    devices: structuredClone(config.devices),
    deviceList: structuredClone(config.deviceList),
    statusSequence,
  });
};

const invoke = async (cmd: string, args?: Record<string, unknown>): Promise<unknown> => {
  await new Promise((r) => setTimeout(r, 30));
  switch (cmd) {
    case 'get_config':
      return structuredClone(config);
    case 'save_config':
      config = structuredClone(args?.config as AppConfig);
      return null;
    case 'get_device_status':
      statusSequence += 1;
      return {
        statuses: { ...statuses },
        stale: false,
        probed: true,
        error: null,
        devices: structuredClone(config.devices),
        deviceList: structuredClone(config.deviceList),
        statusSequence,
      };
    case 'set_device_sort_mode': {
      if (failMode) {
        failMode = false;
        throw { code: 'config.saveFailed', detail: 'harness: disk full' };
      }
      const mode = args?.mode as string;
      config.deviceList.sortMode = mode as DeviceListPrefs['sortMode'];
      bump();
      return structuredClone(snapshot());
    }
    case 'set_device_sort_direction': {
      if (failDirection) {
        failDirection = false;
        throw { code: 'config.saveFailed', detail: 'harness: disk full' };
      }
      const mode = args?.mode as 'status' | 'name' | 'lastConnected';
      const direction = args?.direction as 'asc' | 'desc';
      config.deviceList.directions[mode] = direction;
      bump();
      return structuredClone(snapshot());
    }
    case 'reorder_devices': {
      if (failReorder) {
        failReorder = false;
        throw { code: 'config.saveFailed', detail: 'harness: disk full' };
      }
      const ids = args?.orderedIds as string[];
      config.deviceList.manualOrder = [...ids];
      config.deviceList.sortMode = 'manual';
      bump();
      return structuredClone(snapshot());
    }
    case 'rename_device': {
      const d = config.devices.find((x) => x.id === args?.id);
      if (!d) throw { code: 'device.notFound' };
      d.alias = String(args?.alias);
      bump();
      return structuredClone(config);
    }
    case 'set_wifi_port': {
      const d = config.devices.find((x) => x.id === args?.id);
      if (!d) throw { code: 'device.notFound' };
      if (d.kind !== 'wifi') throw { code: 'wireless.notWireless' };
      const port = Number(args?.port);
      if (!Number.isInteger(port) || port < 1 || port > 65535) {
        throw { code: 'device.invalid', detail: '无线设备的端口不合法' };
      }
      const cid = `${d.ip}:${port}`;
      if (config.devices.some((x) => x.id !== d.id && x.connectionId === cid)) {
        throw { code: 'device.invalid', detail: `连接标识 ${cid} 已被其他设备使用` };
      }
      d.port = port;
      d.connectionId = cid;
      bump();
      return structuredClone(config);
    }
    case 'delete_device': {
      const id = String(args?.id);
      config.devices = config.devices.filter((d) => d.id !== id);
      config.deviceList.manualOrder = config.deviceList.manualOrder.filter((x) => x !== id);
      bump();
      return structuredClone(config);
    }
    case 'connect_device': {
      if (failConnect) {
        const err = failConnect;
        failConnect = null;
        throw err;
      }
      const id = String(args?.request && (args.request as { deviceId: string }).deviceId);
      const d = config.devices.find((x) => x.id === id)!;
      d.lastSessionStartedAt = Date.now();
      updateStatus(id, 'connected');
      sessions.update((list) => [sessionFor(d), ...list]);
      pushStatuses();
      bump();
      return {
        outcomes: [
          {
            deviceId: id,
            alias: d.alias,
            ok: true,
            error: null,
            warning:
              d.alias === '主力机'
                ? { code: 'session.timeSaveFailed', detail: 'harness: disk full' }
                : null,
          },
        ],
        sessions: [sessionFor(d)],
        degraded: false,
        warning: null,
      };
    }
    case 'connect_all': {
      const requests = (args?.requests ?? []) as { deviceId: string }[];
      const outcomes = requests.map(({ deviceId }) => {
        const d = config.devices.find((x) => x.id === deviceId)!;
        updateStatus(deviceId, 'connected');
        return { deviceId, alias: d.alias, ok: true, error: null, warning: null };
      });
      pushStatuses();
      sessions.set(config.devices.map(sessionFor));
      return { outcomes, sessions: config.devices.map(sessionFor), degraded: false, warning: null };
    }
    case 'disconnect_device': {
      const id = String(args?.deviceId);
      updateStatus(id, 'idle');
      sessions.update((list) => list.map((s) => ({ ...s, running: false })));
      pushStatuses();
      return get(sessions);
    }
    case 'disconnect_all':
      for (const id of Object.keys(statuses)) updateStatus(id, 'idle');
      sessions.update((list) => list.map((s) => ({ ...s, running: false })));
      pushStatuses();
      return [];
    case 'list_sessions':
      return [];
    case 'get_session_log':
      return '';
    case 'get_compat_status':
      return { ok: true, detail: null };
    case 'get_env_status':
      return { dir: '/tmp/scrcpy', ready: true, missing: [], version: '3.3.3' };
    case 'list_system_fonts':
      return [];
    case 'get_proxy_list':
      return { direct: 'https://github.com', proxies: [] };
    case 'plugin:webview|set_webview_zoom':
      return null;
    default:
      throw new Error(`harness: unhandled command ${cmd}`);
  }
};

interface HarnessApi {
  lateStatus: () => void;
  failNextReorder: () => void;
  failNextMode: () => void;
  failNextDirection: () => void;
  failNextConnect: () => void;
  state: () => unknown;
  setEditing: (id: string | null) => void;
  deviceIds: () => string[];
  addStatusPush: (id: string, status: DeviceStatus) => void;
  bulkDevices: (count: number) => void;
  scrollTop: () => number;
  setTheme: (theme: 'dark' | 'light') => void;
  setLanguage: (lang: 'zh' | 'en') => void;
  storeIds: () => string[];
  sortedIds: () => string[];
}

declare global {
  interface Window {
    __TAURI_INTERNALS__: {
      invoke: typeof invoke;
      transformCallback: (cb: (payload: unknown) => void) => number;
      metadata?: unknown;
    };
    __harness: HarnessApi;
  }
}

let callbackId = 1;
window.__TAURI_INTERNALS__ = {
  invoke,
  transformCallback: () => callbackId++,
  metadata: { currentWindow: { label: 'main' } },
};

window.__harness = {
  lateStatus: () => {
    // 旧运行态：序号不变（更旧）且状态全部离线；前端必须丢弃。
    const old = statusSequence - 1;
    applyDeviceStatus({
      statuses: Object.fromEntries(config.devices.map((d) => [d.id, 'offline'])) as Record<string, DeviceStatus>,
      stale: false,
      probed: true,
      error: null,
      devices: structuredClone(config.devices),
      deviceList: structuredClone(config.deviceList),
      statusSequence: old,
    });
  },
  failNextReorder: () => {
    failReorder = true;
  },
  failNextMode: () => {
    failMode = true;
  },
  failNextDirection: () => {
    failDirection = true;
  },
  failNextConnect: () => {
    failConnect = {
      code: 'session.linkFailed',
      detail:
        'failed to connect to 192.168.1.5:5555: Connection refused (this is a deliberately long adb-style reason used to verify the row height stays fixed)',
    };
  },
  state: () => ({
    devices: config.devices,
    deviceList: config.deviceList,
    statuses,
  }),
  setEditing: (id) => {
    // 模拟编辑期间的数据推送由测试脚本通过 addStatusPush 完成。
    void id;
  },
  deviceIds: () => config.devices.map((d) => d.id),
  addStatusPush: (id, status) => {
    updateStatus(id, status);
    pushStatuses();
  },
  bulkDevices: (count) => {
    const next: Device[] = Array.from({ length: count }, (_, i) => ({
      id: `bulk-${i}`,
      alias: `批量设备 ${String(i + 1).padStart(2, '0')}`,
      kind: 'usb',
      serial: `BULK-${i}`,
      ip: '',
      port: 0,
      connectionId: `BULK-${i}`,
      lastSessionStartedAt: NOW - (i + 1) * MINUTE,
    }));
    config.devices = next;
    config.deviceList.manualOrder = next.map((d) => d.id);
    config.deviceList.sortMode = 'manual';
    bump();
    statuses = {};
    for (const d of next) statuses[d.id] = 'idle';
    applyDeviceListSnapshot(snapshot());
    pushStatuses();
  },
  scrollTop: () => document.querySelector('.content')?.scrollTop ?? 0,
  setTheme: (theme) => {
    config.ui.theme = theme;
    applyAppearance(config.ui);
  },
  setLanguage: (lang) => {
    config.ui.language = lang;
    applyLanguage(lang);
  },
  storeIds: () => get(devices).map((d) => d.id),
  sortedIds: () => get(sortedDevices).map((d) => d.id),
};

page.set('devices');
// 让 DevicePage 读到真实的外观偏好（密度开关依赖 config store；否则 setter 无处可写）。
configStore.set(structuredClone(config));
sessions.set([sessionFor(baseDevices[1])]);
applyDeviceStatus({
  statuses: { ...statuses },
  stale: false,
  probed: true,
  error: null,
  devices: structuredClone(config.devices),
  deviceList: structuredClone(config.deviceList),
  statusSequence,
});
applyDeviceListSnapshot(snapshot());
selectedDeviceId.set(null);

new DevicePage({ target: document.getElementById('app')! });
new TooltipLayer({ target: document.getElementById('app')! });
