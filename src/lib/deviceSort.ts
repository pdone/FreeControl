import type { Device, DeviceListPrefs, DeviceStatus, SortDirection, SortMode } from './types';

/** 界面语言 → 名称比较用的 locale（显式指定，避免跟随操作系统默认值漂移）。 */
export function sortLocale(lang: 'zh' | 'en'): string {
  return lang === 'zh' ? 'zh' : 'en';
}

const STATUS_RANK: Record<DeviceStatus | 'unknown', number> = {
  connected: 0,
  idle: 1,
  unknown: 2,
  offline: 3,
};

/** 排序模式取值是否合法（后端是权威，前端只用于类型收窄）。 */
export function isSortMode(value: unknown): value is SortMode {
  return (
    value === 'manual' ||
    value === 'status' ||
    value === 'name' ||
    value === 'lastConnected'
  );
}

/** 升序 ↔ 降序（界面再次选择当前方式时使用）。 */
export function flippedDirection(direction: SortDirection): SortDirection {
  return direction === 'asc' ? 'desc' : 'asc';
}

/**
 * 手动顺序索引；不在序列里的设备排在最后（并按登记顺序作为次序）。
 *
 * 后端读入时已经补齐，这里只是防御旧快照与新建设备之间的短暂窗口。
 */
function manualIndexMap(devices: Device[], prefs: DeviceListPrefs): Map<string, number> {
  const map = new Map<string, number>();
  prefs.manualOrder.forEach((id, i) => map.set(id, i));
  devices.forEach((d, i) => {
    if (!map.has(d.id)) map.set(d.id, prefs.manualOrder.length + i);
  });
  return map;
}

/**
 * 按当前排序方式生成显示顺序（不修改输入数组）。
 *
 * 所有模式都以手动顺序为最终比较键：排序键相同的设备位置稳定，
 * 且自动排序永远不写回手动顺序。
 */
export function sortDevices(
  devices: Device[],
  statuses: Record<string, DeviceStatus>,
  prefs: DeviceListPrefs,
  lang: 'zh' | 'en',
): Device[] {
  const manual = manualIndexMap(devices, prefs);
  const byManual = (a: Device, b: Device): number =>
    (manual.get(a.id) ?? Number.MAX_SAFE_INTEGER) -
    (manual.get(b.id) ?? Number.MAX_SAFE_INTEGER);

  if (prefs.sortMode === 'manual') return [...devices].sort(byManual);

  if (prefs.sortMode === 'status') {
    const rank = (d: Device): number => STATUS_RANK[statuses[d.id] ?? 'unknown'];
    const dir = prefs.directions.status === 'desc' ? -1 : 1;
    return [...devices].sort((a, b) => (rank(a) - rank(b)) * dir || byManual(a, b));
  }

  if (prefs.sortMode === 'name') {
    const collator = new Intl.Collator(sortLocale(lang), {
      numeric: true,
      sensitivity: 'accent',
    });
    const dir = prefs.directions.name === 'desc' ? -1 : 1;
    return [...devices].sort(
      (a, b) =>
        collator.compare(a.alias.trim(), b.alias.trim()) * dir || byManual(a, b),
    );
  }

  // lastConnected：方向只决定新旧顺序，无记录始终置底；同时间沿用手动顺序。
  const newestFirst = prefs.directions.lastConnected !== 'asc';
  return [...devices].sort((a, b) => {
    const ta = a.lastSessionStartedAt ?? null;
    const tb = b.lastSessionStartedAt ?? null;
    if (ta === tb) return byManual(a, b);
    if (ta === null) return 1;
    if (tb === null) return -1;
    return (newestFirst ? tb - ta : ta - tb) || byManual(a, b);
  });
}

/**
 * 把 `id` 移到「去掉它之后的序列」的第 `index` 位；返回新数组，不修改输入。
 *
 * 界面的插入点都按这个语义计算：要放到某台设备之前时，`index` 是那台设备
 * 在过滤后的序列里的下标。
 */
export function moveToIndex(ids: string[], id: string, index: number): string[] {
  if (!ids.includes(id)) return [...ids];
  const rest = ids.filter((x) => x !== id);
  const at = Math.max(0, Math.min(index, rest.length));
  rest.splice(at, 0, id);
  return rest;
}

/** 两份 id 序列是否完全一致。 */
export function sameOrder(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((id, i) => id === b[i]);
}
