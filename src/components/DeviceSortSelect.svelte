<script context="module" lang="ts">
  let seq = 0;
</script>

<script lang="ts">
  // 设备页专用的排序方式选择器：四个固定选项，不伪造参数字段、不改动 ParamSelect 的参数模型。
  // 视觉与展开/收起规则沿用参数下拉的 `.sel` 一组类；焦点留在按钮上，选项用 aria-activedescendant。
  import { createEventDispatcher, tick } from 'svelte';
  import { ArrowDown, ArrowUp, ChevronDown } from 'lucide-svelte';
  import type { SortDirection, SortMode } from '../lib/types';

  type SortOption = {
    value: SortMode;
    label: string;
    /** 非手动方式的方向；手动方式没有箭头。 */
    direction?: SortDirection;
    /** 方向的本地化名称，供选项的可访问名称使用。 */
    directionLabel?: string;
  };

  export let value: SortMode;
  export let options: SortOption[];
  export let label: string;
  export let disabled = false;

  const dispatch = createEventDispatcher<{ change: SortMode; toggle: SortMode }>();

  const uid = `dev-sort-${++seq}`;
  const listId = `${uid}-list`;
  const GAP = 4;
  const M = 8;

  let open = false;
  let placed = false;
  let up = false;
  let maxHeight: number | null = null;
  let active = 0;
  let root: HTMLDivElement;
  let trigger: HTMLButtonElement;
  let list: HTMLDivElement | null = null;

  $: selected = options.findIndex((o) => o.value === value);
  $: current = selected >= 0 ? options[selected] : null;
  $: currentText = current?.label ?? '';
  $: currentDirection = current?.direction;
  $: if (disabled && open) close();

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

  function place(): void {
    if (!root || !list) return;
    const r = root.getBoundingClientRect();
    const port = portRect();
    const natural = list.scrollHeight;
    const below = port.bottom - r.bottom - GAP - M;
    const above = r.top - port.top - GAP - M;
    up = below < Math.min(natural, 160) && above > below;
    const available = up ? above : below;
    // 四个选项通常完全放得下；此时不写 max-height，避免「高度恰好等于内容」
    // 时的亚像素取整把列表撑出 1px 溢出，凭空出现滚动条。
    maxHeight = natural > available ? Math.min(natural, Math.max(76, available)) : null;
  }

  function reveal(): void {
    if (!list) return;
    const el = list.children[active] as HTMLElement | undefined;
    if (!el) return;
    const top = el.offsetTop;
    const bottom = top + el.offsetHeight;
    if (top < list.scrollTop) list.scrollTop = top;
    else if (bottom > list.scrollTop + list.clientHeight) list.scrollTop = bottom - list.clientHeight;
  }

  function close(restoreFocus = false): void {
    if (!open) return;
    open = false;
    placed = false;
    if (restoreFocus && trigger) trigger.focus();
  }

  async function openList(dir: 1 | -1 = 1): Promise<void> {
    if (open || disabled || !options.length) return;
    active = selected >= 0 ? selected : dir === 1 ? 0 : options.length - 1;
    open = true;
    placed = false;
    await tick();
    place();
    placed = true;
    reveal();
  }

  function choose(i: number): void {
    const opt = options[i];
    if (!opt) {
      close(true);
      return;
    }
    if (opt.value === value) {
      // 再次选择同一个自动方式 = 翻转它的升降序；手动方式没有方向。
      if (opt.direction) dispatch('toggle', opt.value);
      close(true);
      return;
    }
    dispatch('change', opt.value);
    close(true);
  }

  function move(delta: number): void {
    if (!options.length) return;
    active = (active + delta + options.length) % options.length;
    reveal();
  }

  async function onKey(e: KeyboardEvent): Promise<void> {
    const k = e.key;
    if (disabled) return;
    if (!open) {
      if (k === 'Enter' || k === ' ' || k === 'ArrowDown' || k === 'ArrowUp' || k === 'Home' || k === 'End') {
        e.preventDefault();
        e.stopPropagation();
        await openList(k === 'ArrowUp' || k === 'End' ? -1 : 1);
      }
      return;
    }
    switch (k) {
      case 'ArrowDown':
        e.preventDefault();
        move(1);
        break;
      case 'ArrowUp':
        e.preventDefault();
        move(-1);
        break;
      case 'Home':
        e.preventDefault();
        active = 0;
        reveal();
        break;
      case 'End':
        e.preventDefault();
        active = options.length - 1;
        reveal();
        break;
      case 'Enter':
      case ' ':
        e.preventDefault();
        choose(active);
        break;
      case 'Escape':
        e.preventDefault();
        close(true);
        break;
      case 'Tab':
        close();
        break;
    }
  }

  function onTriggerBlur(e: FocusEvent): void {
    // 焦点移到选项时 mousedown 被拦下，不会触发 blur；这里只处理真正离开。
    if (open && !root.contains(e.relatedTarget as Node | null)) close();
  }

  function onDocPointer(e: PointerEvent): void {
    if (open && root && !root.contains(e.target as Node)) close();
  }
</script>

<svelte:document on:pointerdown={onDocPointer} />

<div class="sel dev-sort" bind:this={root}>
  <button
    class="sel-trigger dev-sort-trigger"
    bind:this={trigger}
    type="button"
    role="combobox"
    aria-label={currentDirection && current?.directionLabel
      ? `${label}: ${currentText} (${current.directionLabel})`
      : `${label}: ${currentText}`}
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-controls={listId}
    aria-activedescendant={open ? `${uid}-opt-${active}` : undefined}
    {disabled}
    on:click={() => (open ? close(true) : openList())}
    on:keydown={onKey}
    on:blur={onTriggerBlur}
  >
    <span class="sel-text dev-sort-value">
      <span class="dev-sort-label">{currentText}</span>
      {#if currentDirection === 'asc'}
        <ArrowUp class="dev-sort-arrow" />
      {:else if currentDirection === 'desc'}
        <ArrowDown class="dev-sort-arrow" />
      {/if}
    </span>
    <ChevronDown />
  </button>
  {#if open}
    <div
      class="sel-list"
      class:on={placed}
      class:up={up}
      id={listId}
      role="listbox"
      aria-label={label}
      style:max-height={maxHeight === null ? undefined : `${maxHeight}px`}
      bind:this={list}
    >
      {#each options as opt, i (opt.value)}
        <button
          type="button"
          class="sel-opt dev-sort-opt"
          class:active={i === active}
          id={`${uid}-opt-${i}`}
          role="option"
          aria-selected={opt.value === value}
          aria-label={opt.direction && opt.directionLabel
            ? `${opt.label}, ${opt.directionLabel}`
            : opt.label}
          tabindex="-1"
          on:mousedown|preventDefault
          on:mouseenter={() => (active = i)}
          on:click={() => choose(i)}
        >
          <span class="dev-sort-opt-label">{opt.label}</span>
          {#if opt.direction === 'asc'}
            <ArrowUp class="dev-sort-arrow" />
          {:else if opt.direction === 'desc'}
            <ArrowDown class="dev-sort-arrow" />
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>
