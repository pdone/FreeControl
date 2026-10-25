<script context="module" lang="ts">
  /** 实例序号：`role="listbox"` 与每个选项的 id 需要全局唯一。 */
  let seq = 0;
</script>

<script lang="ts">
  // 单字体选择器（spec `app-preferences` 的「字体字段的搜索与选择」）。
  //
  // 一个字段只选一个 family；界面字体与代码字体各用一个实例。菜单是「搜索 + 系统默认 +
  // family 列表」，选择后收起；缺字由各区域的系统默认栈兜底。字体列表由 store 共享加载，
  // 两个字段不会重复枚举。
  import { createEventDispatcher, onDestroy, tick } from 'svelte';
  import { Check, ChevronDown, RefreshCw, Search } from 'lucide-svelte';
  import {
    displayFamily,
    fontMatches,
    resolveCssFamily,
  } from '../lib/fonts';
  import { resolvedLang, t } from '../lib/i18n';
  import {
    loadSystemFonts,
    systemFonts,
    systemFontsError,
    systemFontsLoaded,
    systemFontsLoading,
  } from '../lib/store';
  import type { SystemFont } from '../lib/types';

  export let label = '';
  export let value: string | null = null;
  /** 代码字体字段显示「等宽」标记；界面字体字段不显示。 */
  export let showMonoBadge = false;

  const dispatch = createEventDispatcher<{ change: string | null }>();

  const uid = `font-${++seq}`;
  const listId = `${uid}-list`;
  const GAP = 4;
  const M = 8;

  let open = false;
  /** 落位完成前菜单不可见：先量自然高度再决定朝哪边开、限多高。 */
  let placed = false;
  let up = false;
  let maxHeight = 0;
  let active = -1;
  let query = '';
  let lastQuery = '';
  let root: HTMLDivElement;
  let trigger: HTMLButtonElement;
  let panel: HTMLDivElement | null = null;
  let bodyEl: HTMLDivElement | null = null;
  let searchInput: HTMLInputElement | null = null;

  $: filtered = $systemFonts.filter((font) => fontMatches(font, query));
  $: selected = $systemFonts.find((font) => matchesValue(font, value)) ?? null;
  $: currentText = selected
    ? displayFamily(selected, $resolvedLang)
    : (value ?? $t('settings.fonts.systemDefault'));
  $: unavailable =
    !!value &&
    $systemFontsLoaded &&
    !$systemFontsError &&
    selected === null;
  $: note = $systemFontsError
    ? $t('settings.fonts.error')
    : $systemFontsLoading && !$systemFontsLoaded
      ? $t('settings.fonts.loading')
      : unavailable
        ? $t('settings.fonts.unavailable')
        : $systemFontsLoaded && $systemFonts.length === 0
          ? $t('settings.fonts.empty')
          : '';

  $: if (query !== lastQuery) {
    lastQuery = query;
    active = 0;
  }
  $: if (active >= filtered.length) active = Math.max(0, filtered.length - 1);

  /** 族名比较按 CSS 口径不区分大小写；canonical 与别名都算同一个族。 */
  function matchesValue(font: SystemFont, name: string | null): boolean {
    if (!name) return false;
    const needle = name.toLowerCase();
    return [font.family, ...font.aliases].some(
      (candidate) => candidate.toLowerCase() === needle,
    );
  }

  function close(restoreFocus = false): void {
    if (!open) return;
    open = false;
    placed = false;
    unbind();
    if (restoreFocus && trigger) trigger.focus();
  }

  function choose(font: SystemFont): void {
    dispatch('change', resolveCssFamily(font));
    close(true);
  }

  function chooseDefault(): void {
    dispatch('change', null);
    close(true);
  }

  /** 沿祖先链找第一个滚动容器（`.content` 等）；找不到就按视口算。 */
  function portRect(): { top: number; bottom: number } {
    let el: HTMLElement | null = root?.parentElement ?? null;
    while (el) {
      const oy = getComputedStyle(el).overflowY;
      if (oy !== 'visible') {
        const r = el.getBoundingClientRect();
        return { top: r.top, bottom: r.bottom };
      }
      el = el.parentElement;
    }
    return { top: 0, bottom: innerHeight };
  }

  /** 量一次：按滚动容器的可视矩形择向，并把菜单限高到放得下的高度。 */
  function place(): void {
    if (!root || !panel) return;
    const r = root.getBoundingClientRect();
    const port = portRect();
    const chrome = panel.offsetHeight - (bodyEl?.clientHeight ?? 0);
    const natural = (bodyEl?.scrollHeight ?? 0) + Math.max(0, chrome);
    const below = port.bottom - r.bottom - GAP - M;
    const above = r.top - port.top - GAP - M;
    up = below < Math.min(natural, 280) && above > below;
    const room = Math.max(140, up ? above : below);
    maxHeight = Math.min(natural, room);
  }

  /** 把当前候选滚进列表的可见区（不用 scrollIntoView —— 它会连页面滚动一起带）。 */
  function reveal(): void {
    if (!bodyEl || active < 0) return;
    const el = bodyEl.querySelector<HTMLElement>(`[data-opt="${active}"]`);
    if (!el) return;
    const top = el.offsetTop;
    const bottom = top + el.offsetHeight;
    if (top < bodyEl.scrollTop) bodyEl.scrollTop = top;
    else if (bottom > bodyEl.scrollTop + bodyEl.clientHeight) {
      bodyEl.scrollTop = bottom - bodyEl.clientHeight;
    }
  }

  async function openPanel(): Promise<void> {
    if (open) {
      close();
      return;
    }
    open = true;
    placed = false;
    query = '';
    active = -1;
    bind();
    await tick();
    // preventScroll：避免浏览器为露出搜索框而滚动页面，那个滚动会被「页面滚动即收起」监听当成用户滚动。
    searchInput?.focus({ preventScroll: true });
    if (!$systemFontsLoaded) await loadSystemFonts(false);
    await tick();
    active = filtered.length ? 0 : -1;
    place();
    placed = true;
  }

  async function onSearchKey(e: KeyboardEvent): Promise<void> {
    const k = e.key;
    if (k === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close(true);
      return;
    }
    if (k === 'Tab') {
      close();
      return;
    }
    if (!filtered.length) return;

    switch (k) {
      case 'ArrowDown':
        e.preventDefault();
        active = Math.min(active + 1, filtered.length - 1);
        reveal();
        break;
      case 'ArrowUp':
        e.preventDefault();
        active = Math.max(active - 1, 0);
        reveal();
        break;
      case 'Home':
        e.preventDefault();
        active = 0;
        reveal();
        break;
      case 'End':
        e.preventDefault();
        active = filtered.length - 1;
        reveal();
        break;
      case 'Enter': {
        e.preventDefault();
        const font = filtered[active];
        if (font) choose(font);
        break;
      }
      default:
        return;
    }
    e.stopPropagation();
  }

  function bind(): void {
    document.addEventListener('pointerdown', onDocPointerDown, true);
    window.addEventListener('scroll', onScroll, true);
    window.addEventListener('resize', onResize);
    window.addEventListener('keydown', onWinKey);
  }

  function unbind(): void {
    document.removeEventListener('pointerdown', onDocPointerDown, true);
    window.removeEventListener('scroll', onScroll, true);
    window.removeEventListener('resize', onResize);
    window.removeEventListener('keydown', onWinKey);
  }

  function onDocPointerDown(e: PointerEvent): void {
    if (root && !root.contains(e.target as Node)) close();
  }

  function onScroll(e: Event): void {
    if (panel && e.target instanceof Node && panel.contains(e.target)) return;
    close();
  }

  function onResize(): void {
    close();
  }

  function onWinKey(e: KeyboardEvent): void {
    if (e.key === 'Escape' && open) {
      e.stopPropagation();
      close(true);
    }
  }

  onDestroy(unbind);
</script>

<div class="font-picker" bind:this={root}>
  <button
    bind:this={trigger}
    class="sel-trigger"
    type="button"
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-controls={open ? listId : undefined}
    aria-label={label || $t('settings.fonts.systemDefault')}
    on:click={openPanel}
  >
    <span class="sel-text">{currentText}</span>
    <ChevronDown />
  </button>

  {#if open}
    <div
      class="font-menu"
      class:on={placed}
      class:up
      bind:this={panel}
      style={maxHeight ? `max-height:${maxHeight}px` : undefined}
    >
      <div class="font-search">
        <Search />
        <input
          bind:this={searchInput}
          bind:value={query}
          type="search"
          placeholder={$t('settings.fonts.search')}
          aria-label={$t('settings.fonts.search')}
          aria-controls={listId}
          aria-activedescendant={active >= 0 ? `${uid}-opt-${active}` : undefined}
          on:input={() => {
            active = 0;
            void tick().then(reveal);
          }}
          on:keydown={onSearchKey}
        />
        <button
          class="font-refresh"
          type="button"
          title={$t('settings.fonts.refresh')}
          aria-label={$t('settings.fonts.refresh')}
          disabled={$systemFontsLoading}
          on:click={() => void loadSystemFonts(true)}
        >
          <RefreshCw class={$systemFontsLoading ? 'font-spin' : ''} />
        </button>
      </div>

      <button
        class="font-default"
        class:picked={value === null}
        type="button"
        on:click={chooseDefault}
      >
        <span class="font-check">{#if value === null}<Check />{/if}</span>
        <span class="font-name">{$t('settings.fonts.systemDefault')}</span>
      </button>

      <div
        class="font-body"
        bind:this={bodyEl}
        id={listId}
        role="listbox"
        aria-label={label}
      >
        {#if filtered.length}
          {#each filtered as font, i (font.family)}
            {@const isSelected = matchesValue(font, value)}
            <button
              class="font-opt"
              class:active={i === active}
              class:picked={isSelected}
              type="button"
              role="option"
              tabindex="-1"
              id={`${uid}-opt-${i}`}
              data-opt={i}
              aria-selected={isSelected}
              on:mousedown|preventDefault
              on:click={() => choose(font)}
              on:mousemove={() => (active = i)}
            >
              <span class="font-check">{#if isSelected}<Check />{/if}</span>
              <span class="font-name">{displayFamily(font, $resolvedLang)}</span>
              {#if showMonoBadge && font.monospaced}
                <span class="font-mono-tag">{$t('settings.fonts.mono')}</span>
              {/if}
            </button>
          {/each}
        {:else if query.trim()}
          <div class="font-empty">{$t('settings.fonts.noMatch')}</div>
        {/if}
      </div>
    </div>
  {/if}
</div>

{#if note}
  <div class="font-note" class:err={!!$systemFontsError || unavailable}>
    <span>{note}</span>
    {#if $systemFontsError}
      <button type="button" on:click={() => void loadSystemFonts(true)}>
        {$t('settings.fonts.retry')}
      </button>
    {/if}
  </div>
{/if}
