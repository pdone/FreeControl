// 应用列表的本地活动状态：固定与打开记录。都按设备存放在界面本地存储；
// 任何读写失败、损坏或版本不符都静默降级为「没有记录」，不影响清单读取与打开应用。
import type { CacheStorage } from './appListCache';

export const APP_PINS_KEY = 'freecontrol-app-pins';
export const APP_USAGE_KEY = 'freecontrol-app-usage';

const VERSION = 1;

export interface AppUsageRecord {
  lastOpenedAt: number;
  launchCount: number;
}

/** 设备 ID -> 固定包名列表；顺序沿用加入固定的先后。 */
export type AppPins = Record<string, string[]>;
/** 设备 ID -> 包名 -> 最近打开时间与累计次数。 */
export type AppUsage = Record<string, Record<string, AppUsageRecord>>;

function defaultStorage(): CacheStorage | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage;
  } catch {
    return null;
  }
}

function isObject(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}

function drop(storage: CacheStorage, key: string): void {
  try {
    storage.removeItem(key);
  } catch {
    // 删不掉也只是下次仍按损坏处理。
  }
}

/** 读取并校验公共外壳（版本 + devices 对象）；缺失或损坏时删键并返回 null。 */
function readDevices(
  key: string,
  storage: CacheStorage | null,
): Record<string, unknown> | null {
  if (!storage) return null;
  try {
    const raw = storage.getItem(key);
    if (!raw) return null;
    let parsed: unknown;
    try {
      parsed = JSON.parse(raw);
    } catch {
      drop(storage, key);
      return null;
    }
    if (!isObject(parsed) || parsed.v !== VERSION || !isObject(parsed.devices)) {
      drop(storage, key);
      return null;
    }
    return parsed.devices;
  } catch {
    return null;
  }
}

function writePayload(key: string, devices: unknown, storage: CacheStorage | null): void {
  if (!storage) return;
  try {
    storage.setItem(key, JSON.stringify({ v: VERSION, devices }));
  } catch {
    // 配额不足等：留给内存状态。
  }
}

/** 读取固定记录；损坏、未知版本或存储不可用时返回空表并（能删则）删键。 */
export function readAppPins(storage: CacheStorage | null = defaultStorage()): AppPins {
  const devices = readDevices(APP_PINS_KEY, storage);
  if (!devices || !storage) return {};
  const out: AppPins = {};
  for (const [deviceId, value] of Object.entries(devices)) {
    if (!deviceId || !Array.isArray(value)) continue;
    const list: string[] = [];
    const seen = new Set<string>();
    for (const pkg of value) {
      if (typeof pkg === 'string' && pkg !== '' && !seen.has(pkg)) {
        seen.add(pkg);
        list.push(pkg);
      }
    }
    if (list.length) out[deviceId] = list;
  }
  if (Object.keys(out).length === 0 && Object.keys(devices).length > 0) {
    drop(storage, APP_PINS_KEY);
  }
  return out;
}

export function writeAppPins(pins: AppPins, storage: CacheStorage | null = defaultStorage()): void {
  writePayload(APP_PINS_KEY, pins, storage);
}

function isUsageRecord(value: unknown): value is AppUsageRecord {
  if (!isObject(value)) return false;
  return (
    typeof value.lastOpenedAt === 'number' &&
    Number.isFinite(value.lastOpenedAt) &&
    value.lastOpenedAt >= 0 &&
    typeof value.launchCount === 'number' &&
    Number.isInteger(value.launchCount) &&
    value.launchCount >= 0
  );
}

/** 读取打开记录；非法条目丢弃，整表无有效项时按损坏处理。 */
export function readAppUsage(storage: CacheStorage | null = defaultStorage()): AppUsage {
  const devices = readDevices(APP_USAGE_KEY, storage);
  if (!devices || !storage) return {};
  const out: AppUsage = {};
  for (const [deviceId, value] of Object.entries(devices)) {
    if (!deviceId || !isObject(value)) continue;
    const device: Record<string, AppUsageRecord> = {};
    for (const [pkg, record] of Object.entries(value)) {
      if (pkg && isUsageRecord(record)) {
        device[pkg] = { lastOpenedAt: record.lastOpenedAt, launchCount: record.launchCount };
      }
    }
    if (Object.keys(device).length) out[deviceId] = device;
  }
  if (Object.keys(out).length === 0 && Object.keys(devices).length > 0) {
    drop(storage, APP_USAGE_KEY);
  }
  return out;
}

export function writeAppUsage(
  usage: AppUsage,
  storage: CacheStorage | null = defaultStorage(),
): void {
  writePayload(APP_USAGE_KEY, usage, storage);
}

/** 固定的纯更新：不修改入参，空列表时移除该设备键。 */
export function withPin(
  pins: AppPins,
  deviceId: string,
  pkg: string,
  fixed: boolean,
): AppPins {
  const current = pins[deviceId] ?? [];
  const list = fixed
    ? current.includes(pkg)
      ? current
      : [...current, pkg]
    : current.filter((item) => item !== pkg);
  const next = { ...pins };
  if (list.length) next[deviceId] = list;
  else delete next[deviceId];
  return next;
}

/** 打开一次的纯更新：累加次数并把最近打开时间写成 now。 */
export function withAppLaunch(
  usage: AppUsage,
  deviceId: string,
  pkg: string,
  now = Date.now(),
): AppUsage {
  const device = { ...(usage[deviceId] ?? {}) };
  const prev = device[pkg];
  device[pkg] = { lastOpenedAt: now, launchCount: (prev?.launchCount ?? 0) + 1 };
  return { ...usage, [deviceId]: device };
}

export function withoutDevicePins(pins: AppPins, deviceId: string): AppPins {
  if (!(deviceId in pins)) return pins;
  const next = { ...pins };
  delete next[deviceId];
  return next;
}

export function withoutDeviceUsage(usage: AppUsage, deviceId: string): AppUsage {
  if (!(deviceId in usage)) return usage;
  const next = { ...usage };
  delete next[deviceId];
  return next;
}
