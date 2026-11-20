// 应用列表的纯排序：固定组始终在前，组内按当前排序方式排列。
// 默认方式保留读取顺序；最近打开按时间倒序，未打开过的时间按 0 处理，
// 稳定排序因此会保留它们的原相对顺序并整体排在已打开应用之后。
import type { AppEntry, SortDirection } from './types';
import type { AppSortDirections, AppSortMode } from './appListCache';
import type { AppUsageRecord } from './appActivity';

export function sortAppEntries(
  list: readonly AppEntry[],
  mode: AppSortMode,
  directions: AppSortDirections,
  lang: string,
  pinned: ReadonlySet<string>,
  usage: Record<string, AppUsageRecord>,
): AppEntry[] {
  const pinnedItems: AppEntry[] = [];
  const rest: AppEntry[] = [];
  for (const entry of list) {
    (pinned.has(entry.package) ? pinnedItems : rest).push(entry);
  }

  const sortGroup = (items: AppEntry[]): AppEntry[] => {
    if (mode === 'default') return items;
    if (mode === 'recent') {
      return items.slice().sort((a, b) => {
        const at = usage[a.package]?.lastOpenedAt ?? 0;
        const bt = usage[b.package]?.lastOpenedAt ?? 0;
        return bt - at;
      });
    }
    const collator = new Intl.Collator(lang === 'en' ? 'en' : 'zh-CN', {
      sensitivity: 'base',
      numeric: true,
    });
    const key: 'name' | 'package' = mode === 'name' ? 'name' : 'package';
    const direction: SortDirection = directions[key];
    const sign = direction === 'desc' ? -1 : 1;
    return items.slice().sort((a, b) => collator.compare(a[key], b[key]) * sign);
  };

  return [...sortGroup(pinnedItems), ...sortGroup(rest)];
}
