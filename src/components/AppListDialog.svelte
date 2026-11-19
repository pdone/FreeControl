<script lang="ts">
  // 应用列表弹窗（app-list-launch）：外壳与设备参数弹窗同款，体区固定「排序 / 搜索 / 刷新」
  // 工具栏与可滚动列表。清单按设备缓存在 store（本次运行内），排序也只保持本次运行。
  import { createEventDispatcher, onMount } from 'svelte';
  import { fade } from 'svelte/transition';
  import { DIALOG_FADE, DIALOG_SCALE, dialogScale } from '../lib/motion';
  import { backdropClose } from '../lib/backdrop';
  import {
    appLists,
    appListLoading,
    appListErrors,
    loadDeviceApps,
    openDeviceApp,
    appSortMode,
    appSortDirections,
    setAppSort,
    type AppSortMode,
  } from '../lib/store';
  import type { AppSortDirections } from '../lib/appListCache';
  import { t, resolvedLang } from '../lib/i18n';
  import { errorTextOf } from '../lib/message';
  import { tip } from '../lib/tooltip';
  import type { AppEntry, AppOpenMode, SortDirection } from '../lib/types';
  import {
    X,
    Maximize2,
    Minimize2,
    RefreshCw,
    Search,
    LayoutGrid,
    ArrowUp,
    ArrowDown,
    ChevronDown,
    Monitor,
    AppWindow,
    AlertTriangle,
  } from 'lucide-svelte';

  export let deviceId: string;

  const dispatch = createEventDispatcher<{ close: void }>();

  let expanded = false;
  let keyword = '';
  let sortOpen = false;
  let sortActive = 0;
  let sortEl: HTMLDivElement;
  let sortTrigger: HTMLButtonElement;
  let pendingKey: string | null = null;
  let feedback = '';
  let notice = '';
  let actionError: unknown = null;

  type SortOption = {
    value: AppSortMode;
    key: 'appList.sort.default' | 'appList.sort.name' | 'appList.sort.package';
    /** 默认排序没有方向；按名称 / 按包名各自显示自己保存的方向。 */
    direction: SortDirection | null;
  };

  let sortOptions: SortOption[];
  $: sortOptions = [
    { value: 'default', key: 'appList.sort.default', direction: null },
    { value: 'name', key: 'appList.sort.name', direction: $appSortDirections.name },
    { value: 'package', key: 'appList.sort.package', direction: $appSortDirections.package },
  ];

  $: cached = $appLists[deviceId];
  $: loading = $appListLoading[deviceId] ?? false;
  $: loadError = $appListErrors[deviceId];
  $: hasCache = cached !== undefined;

  $: sortLabel =
    $appSortMode === 'default'
      ? $t('appList.sort.default')
      : $appSortMode === 'name'
        ? $t('appList.sort.name')
        : $t('appList.sort.package');
  $: currentDirection = sortOptions.find((o) => o.value === $appSortMode)?.direction ?? null;

  $: sorted = sortApps(cached ?? [], $appSortMode, $appSortDirections, $resolvedLang);
  $: filtered = keyword.trim()
    ? sorted.filter((a) => {
        const q = keyword.trim().toLowerCase();
        return a.name.toLowerCase().includes(q) || a.package.toLowerCase().includes(q);
      })
    : sorted;

  function sortApps(
    list: AppEntry[],
    mode: AppSortMode,
    directions: AppSortDirections,
    lang: string,
  ): AppEntry[] {
    const out = list.slice();
    if (mode === 'default') return out;
    const key: 'name' | 'package' = mode === 'name' ? 'name' : 'package';
    const collator = new Intl.Collator(lang === 'en' ? 'en' : 'zh-CN', {
      sensitivity: 'base',
      numeric: true,
    });
    const dir = directions[mode] === 'desc' ? -1 : 1;
    out.sort(
      (a, b) => collator.compare(a[key], b[key]) * dir,
    );
    return out;
  }

  function chooseSort(mode: AppSortMode): void {
    const next = { ...$appSortDirections };
    // 再次选择当前方式只翻转它自己的方向；选择另一方式沿用该方式自己保存的方向。
    if (mode !== 'default' && mode === $appSortMode) {
      next[mode] = next[mode] === 'asc' ? 'desc' : 'asc';
    }
    setAppSort(mode, next);
    sortOpen = false;
    sortTrigger?.focus();
  }

  function openSort(dir = 1): void {
    sortActive = sortOptions.findIndex((o) => o.value === $appSortMode);
    if (sortActive < 0) sortActive = 0;
    if (dir === -1) sortActive = sortOptions.length - 1;
    sortOpen = true;
  }

  function toggleSort(): void {
    if (sortOpen) sortOpen = false;
    else openSort();
  }

  function onSortKeydown(e: KeyboardEvent): void {
    const k = e.key;
    if (!sortOpen) {
      if (k === 'Enter' || k === ' ' || k === 'ArrowDown' || k === 'ArrowUp' || k === 'Home' || k === 'End') {
        if (k !== 'Enter') e.preventDefault();
        e.stopPropagation();
        openSort(k === 'ArrowUp' || k === 'End' ? -1 : 1);
      }
      return;
    }
    if (['ArrowDown', 'ArrowUp', 'Home', 'End', 'Enter', ' ', 'Escape', 'Tab'].includes(k)) {
      e.stopPropagation();
    }
    switch (k) {
      case 'ArrowDown':
        e.preventDefault();
        sortActive = (sortActive + 1) % sortOptions.length;
        break;
      case 'ArrowUp':
        e.preventDefault();
        sortActive = (sortActive - 1 + sortOptions.length) % sortOptions.length;
        break;
      case 'Home':
        e.preventDefault();
        sortActive = 0;
        break;
      case 'End':
        e.preventDefault();
        sortActive = sortOptions.length - 1;
        break;
      case 'Enter':
      case ' ':
        e.preventDefault();
        chooseSort(sortOptions[sortActive].value);
        break;
      case 'Escape':
        e.preventDefault();
        sortOpen = false;
        break;
      case 'Tab':
        sortOpen = false;
        break;
    }
  }

  function onWindowClick(e: MouseEvent): void {
    if (sortOpen && sortEl && !sortEl.contains(e.target as Node)) sortOpen = false;
  }

  function close(): void {
    dispatch('close');
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key !== 'Escape') return;
    if (sortOpen) {
      sortOpen = false;
      return;
    }
    close();
  }

  async function refresh(manual = true): Promise<void> {
    actionError = null;
    feedback = '';
    try {
      await loadDeviceApps(deviceId, manual);
    } catch {
      // 错误由 store 上的 appListErrors 呈现；这里不再重复写一份。
    }
  }

  async function open(entry: AppEntry, mode: AppOpenMode): Promise<void> {
    if (pendingKey) return;
    const key = `${entry.package}:${mode}`;
    pendingKey = key;
    feedback = '';
    notice = '';
    actionError = null;
    try {
      const res = await openDeviceApp(deviceId, entry.package, mode);
      feedback =
        mode === 'newWindow'
          ? $t('appList.feedback.newWindow', { name: entry.name })
          : res.startedWindow
            ? $t('appList.feedback.started', { name: entry.name })
            : $t('appList.feedback.current', { name: entry.name });
      if (res.notice) notice = errorTextOf(res.notice, $t);
      // 打开成功后弹窗保持：只更新反馈，由用户手动关闭，便于连续打开多个应用。
    } catch (e) {
      actionError = e;
    } finally {
      pendingKey = null;
    }
  }

  onMount(() => {
    if (!hasCache) void refresh(false);
  });
</script>

<svelte:window on:keydown={onKeydown} on:click={onWindowClick} />

<div class="overlay dp-overlay app-list-overlay show" transition:fade={DIALOG_FADE}>
  <div class="overlay-backdrop" role="presentation" use:backdropClose={close} />
  <div
    class="modal dp-modal app-list-modal"
    class:expanded
    transition:dialogScale={DIALOG_SCALE}
    role="dialog"
    aria-modal="true"
    aria-label={$t('device.appList')}
  >
    <header class="dp-head" data-tauri-drag-region="deep">
      <span class="dp-title">
        <LayoutGrid />
        <span class="dp-title-text">{$t('device.appList')}</span>
      </span>
      <span class="dp-actions">
        <span class="tip-host" use:tip={expanded ? $t('device.params.collapse') : $t('device.params.expand')}>
          <button
            class="dp-icon-btn"
            type="button"
            aria-label={expanded ? $t('device.params.collapse') : $t('device.params.expand')}
            aria-pressed={expanded}
            on:click={() => (expanded = !expanded)}
          >
            {#if expanded}<Minimize2 />{:else}<Maximize2 />{/if}
          </button>
        </span>
        <button class="dp-icon-btn" type="button" aria-label={$t('about.close')} on:click={close}>
          <X />
        </button>
      </span>
    </header>

    <div class="dp-body al-body">
      <div class="al-toolbar">
        <div class="sel al-sort" bind:this={sortEl} data-v={$appSortMode}>
          <button
            class="sel-trigger"
            type="button"
            bind:this={sortTrigger}
            role="combobox"
            aria-label={$t('appList.sort.label')}
            aria-haspopup="listbox"
            aria-expanded={sortOpen}
            aria-controls="alSortList"
            on:click|stopPropagation={toggleSort}
            on:keydown={onSortKeydown}
          >
            <span class="al-sort-value">
              <span class="al-sort-label">{sortLabel}</span>
              {#if currentDirection}
                <span class="al-sort-dir" aria-hidden="true">
                  {#if currentDirection === 'desc'}<ArrowDown />{:else}<ArrowUp />{/if}
                </span>
              {/if}
            </span>
            <ChevronDown />
          </button>
          {#if sortOpen}
            <div class="sel-list on al-sort-list" id="alSortList" role="listbox" aria-label={$t('appList.sort.label')}>
              {#each sortOptions as o, i}
                <button
                  class="sel-opt"
                  class:active={i === sortActive}
                  type="button"
                  role="option"
                  aria-selected={o.value === $appSortMode}
                  aria-label={o.direction
                    ? `${$t(o.key)}, ${$t(o.direction === 'asc' ? 'device.sort.ascending' : 'device.sort.descending')}`
                    : $t(o.key)}
                  tabindex="-1"
                  on:mousedown|preventDefault
                  on:mousemove={() => (sortActive = i)}
                  on:click={() => chooseSort(o.value)}
                >
                  <span>{$t(o.key)}</span>
                  {#if o.direction}
                    <span class="al-sort-dir" aria-hidden="true">
                      {#if o.direction === 'desc'}<ArrowDown />{:else}<ArrowUp />{/if}
                    </span>
                  {/if}
                </button>
              {/each}
            </div>
          {/if}
        </div>

        <label class="al-search">
          <Search />
          <input
            type="search"
            bind:value={keyword}
            placeholder={$t('appList.searchPlaceholder')}
            autocomplete="off"
          />
        </label>

        <button
          class="btn ghost sm al-refresh"
          class:busy={loading}
          type="button"
          disabled={loading}
          on:click={() => refresh(true)}
        >
          <RefreshCw />
          {$t('appList.refresh')}
        </button>
      </div>

      {#if feedback}
        <div class="al-feedback">{feedback}</div>
      {/if}
      {#if notice}
        <div class="al-feedback notice">{notice}</div>
      {/if}
      {#if actionError}
        <div class="al-error">
          <AlertTriangle />
          <span>{errorTextOf(actionError, $t)}</span>
        </div>
      {/if}
      {#if loadError && hasCache}
        <div class="al-error">
          <AlertTriangle />
          <span>{errorTextOf(loadError, $t)}</span>
        </div>
      {/if}

      <div class="al-list">
        {#if loading && !hasCache}
          <div class="al-state">
            <span class="status-spinner" aria-hidden="true"></span>
            <span>{$t('appList.loading')}</span>
          </div>
        {:else if loadError && !hasCache}
          <div class="al-state">
            <span>{errorTextOf(loadError, $t)}</span>
            <button class="btn ghost sm" type="button" on:click={() => refresh(true)}>
              {$t('appList.retry')}
            </button>
          </div>
        {:else if cached && cached.length === 0}
          <div class="al-state">{$t('appList.empty')}</div>
        {:else if cached && filtered.length === 0}
          <div class="al-state">{$t('appList.noMatch')}</div>
        {:else if cached}
          {#each filtered as entry (entry.package)}
            <div class="al-row">
              <span class="al-name">
                <b>{entry.name}</b>
                <span class="al-pkg">（{entry.package}）</span>
              </span>
              <span class="al-actions">
                <button
                  class="btn ghost sm"
                  class:busy={pendingKey === `${entry.package}:currentWindow`}
                  type="button"
                  disabled={pendingKey !== null}
                  aria-label={$t('appList.openCurrent')}
                  use:tip={$t('appList.openCurrentTip')}
                  on:click={() => open(entry, 'currentWindow')}
                >
                  {#if pendingKey === `${entry.package}:currentWindow`}
                    <span class="status-spinner" aria-hidden="true"></span>
                  {:else}
                    <Monitor />
                  {/if}
                  {$t('appList.openCurrent')}
                </button>
                <button
                  class="btn ghost sm"
                  class:busy={pendingKey === `${entry.package}:newWindow`}
                  type="button"
                  disabled={pendingKey !== null}
                  aria-label={$t('appList.openNew')}
                  use:tip={$t('appList.openNewTip')}
                  on:click={() => open(entry, 'newWindow')}
                >
                  {#if pendingKey === `${entry.package}:newWindow`}
                    <span class="status-spinner" aria-hidden="true"></span>
                  {:else}
                    <AppWindow />
                  {/if}
                  {$t('appList.openNew')}
                </button>
              </span>
            </div>
          {/each}
        {/if}
      </div>
    </div>
  </div>
</div>
