// 应用清单的跨重启缓存（localStorage）。缓存只用于加速展示：任何读写失败都按
// 「未命中 / 未写入」处理，不能影响清单读取与打开应用。
import type { AppEntry } from './types';

export const APP_LIST_CACHE_TTL_MS = 7 * 24 * 60 * 60 * 1000;

const KEY_PREFIX = 'freecontrol-app-list:';
const CACHE_VERSION = 1;

/** 缓存只用到这三个方法，测试可注入内存桩。 */
export type CacheStorage = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;

interface CachedPayload {
  v: number;
  savedAt: number;
  apps: AppEntry[];
}

export function appListCacheKey(deviceId: string): string {
  return `${KEY_PREFIX}${deviceId}`;
}

function defaultStorage(): CacheStorage | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage;
  } catch {
    return null;
  }
}

function isEntry(value: unknown): value is AppEntry {
  if (!value || typeof value !== 'object') return false;
  const entry = value as Record<string, unknown>;
  return (
    typeof entry.name === 'string' &&
    entry.name !== '' &&
    typeof entry.package === 'string' &&
    entry.package !== '' &&
    typeof entry.system === 'boolean'
  );
}

function drop(storage: CacheStorage, key: string): void {
  try {
    storage.removeItem(key);
  } catch {
    // 删不掉也只是下次仍按损坏处理。
  }
}

/** 读取某设备的缓存；过期、损坏或存储不可用时返回 `null`。 */
export function readCachedApps(
  deviceId: string,
  now = Date.now(),
  storage: CacheStorage | null = defaultStorage(),
): AppEntry[] | null {
  if (!storage) return null;
  const key = appListCacheKey(deviceId);
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
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      drop(storage, key);
      return null;
    }
    const payload = parsed as Record<string, unknown>;
    if (
      payload.v !== CACHE_VERSION ||
      typeof payload.savedAt !== 'number' ||
      !Number.isFinite(payload.savedAt) ||
      !Array.isArray(payload.apps)
    ) {
      drop(storage, key);
      return null;
    }
    if (now - payload.savedAt > APP_LIST_CACHE_TTL_MS) {
      drop(storage, key);
      return null;
    }
    const apps = payload.apps.filter(isEntry);
    // 原本有内容却全部不合法时按损坏处理，避免把损坏读成「空清单」。
    if (apps.length === 0 && payload.apps.length > 0) {
      drop(storage, key);
      return null;
    }
    return apps;
  } catch {
    return null;
  }
}

/** 写入某设备的缓存；失败静默丢弃（本次仍使用内存缓存）。 */
export function writeCachedApps(
  deviceId: string,
  apps: AppEntry[],
  now = Date.now(),
  storage: CacheStorage | null = defaultStorage(),
): void {
  if (!storage) return;
  const payload: CachedPayload = { v: CACHE_VERSION, savedAt: now, apps };
  try {
    storage.setItem(appListCacheKey(deviceId), JSON.stringify(payload));
  } catch {
    // 配额不足等：留给内存缓存。
  }
}

/** 删除某设备的缓存（删除设备或判为损坏时）。 */
export function removeCachedApps(
  deviceId: string,
  storage: CacheStorage | null = defaultStorage(),
): void {
  if (!storage) return;
  drop(storage, appListCacheKey(deviceId));
}

export type AppSortMode = 'default' | 'name' | 'package';

export const APP_SORT_CACHE_KEY = 'freecontrol-ui-app-sort';

export interface AppSortPreference {
  mode: AppSortMode;
  desc: boolean;
}

const SORT_MODES: readonly string[] = ['default', 'name', 'package'];

function isSortMode(value: unknown): value is AppSortMode {
  return typeof value === 'string' && SORT_MODES.includes(value);
}

/** 读取排序偏好；缺失、损坏或取值非法时返回 `null`（调用方回默认排序）。 */
export function readAppSortPreference(
  storage: CacheStorage | null = defaultStorage(),
): AppSortPreference | null {
  if (!storage) return null;
  try {
    const raw = storage.getItem(APP_SORT_CACHE_KEY);
    if (!raw) return null;
    let parsed: unknown;
    try {
      parsed = JSON.parse(raw);
    } catch {
      drop(storage, APP_SORT_CACHE_KEY);
      return null;
    }
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      drop(storage, APP_SORT_CACHE_KEY);
      return null;
    }
    const payload = parsed as Record<string, unknown>;
    if (
      payload.v !== CACHE_VERSION ||
      !isSortMode(payload.mode) ||
      typeof payload.desc !== 'boolean'
    ) {
      drop(storage, APP_SORT_CACHE_KEY);
      return null;
    }
    return { mode: payload.mode, desc: payload.desc };
  } catch {
    return null;
  }
}

/** 写入排序偏好；失败静默丢弃（本次运行内的选择仍然生效）。 */
export function writeAppSortPreference(
  mode: AppSortMode,
  desc: boolean,
  storage: CacheStorage | null = defaultStorage(),
): void {
  if (!storage) return;
  try {
    storage.setItem(APP_SORT_CACHE_KEY, JSON.stringify({ v: CACHE_VERSION, mode, desc }));
  } catch {
    // 配额不足等：不影响本次排序。
  }
}
