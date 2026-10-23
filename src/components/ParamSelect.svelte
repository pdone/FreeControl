<script context="module" lang="ts">
  /** 实例序号：`role="listbox"` 与每个选项的 id 需要全局唯一（同页有 12 个下拉）。 */
  let seq = 0;
</script>

<script lang="ts">
  // 下拉参数的自绘列表（design D1–D3）：宿主原生 `<select>` 的弹出列表由 WebView 自己绘制，
  // 页面层叠取值对它无效（帮助面板压不过它），故改为「按钮 + 页面内列表」。
  //
  // 焦点**始终留在按钮上**，当前项用 `aria-activedescendant` 表达；选项是 `tabindex="-1"` 的按钮
  // （鼠标可点、Tab 进不去），`mousedown` 一律 preventDefault，免得点选项时焦点被挪走。
  import { createEventDispatcher, onDestroy, tick } from 'svelte';
  import { ChevronDown } from 'lucide-svelte';
  import type { ParamField } from '../lib/params';
  import { t } from '../lib/i18n';
  import { labelText } from '../lib/paramText';

  export let field: ParamField;
  export let value: unknown;

  const dispatch = createEventDispatcher<{ change: string }>();

  const uid = `sel-${++seq}`;
  const listId = `${uid}-list`;
  /** 列表与按钮之间的缝、滚动容器边缘的余量（与提示层的 M 同值）。 */
  const GAP = 4;
  const M = 8;

  let open = false;
  /** 落位完成前列表不可见：先量自然高度再决定朝哪边开、限多高，避免在错的位置闪一帧。 */
  let placed = false;
  let up = false;
  let maxHeight = 0;
  let active = 0;
  let root: HTMLDivElement;
  let trigger: HTMLButtonElement;
  let list: HTMLDivElement | null = null;

  /** 首字母定位的连击缓冲（700ms 内连续输入按前缀匹配，否则从这一下重新开始）。 */
  const typed = { text: '', at: 0 };

  $: options = field.options ?? [];
  $: label = labelText(field, $t);
  $: selected = options.findIndex((o) => o.value === String(value ?? ''));
  $: currentText =
    selected >= 0
      ? labelText(options[selected], $t)
      : value === null || value === undefined
        ? ''
        : String(value);

  function close(restoreFocus = false): void {
    if (!open) return;
    open = false;
    placed = false;
    unbind();
    if (restoreFocus && trigger) trigger.focus();
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

  /** 量一次：按滚动容器的可视矩形择向，并把列表限高到放得下的高度。 */
  function place(): void {
    if (!root || !list) return;
    const r = root.getBoundingClientRect();
    const port = portRect();
    const natural = list.scrollHeight;
    const below = port.bottom - r.bottom - GAP - M;
    const above = r.top - port.top - GAP - M;
    up = below < Math.min(natural, 220) && above > below;
    const room = Math.max(76, up ? above : below);
    maxHeight = Math.min(natural, room);
  }

  /** 把当前项滚进列表的可见区（不用 scrollIntoView —— 它会连页面滚动一起带）。 */
  function reveal(): void {
    if (!list) return;
    const el = list.children[active] as HTMLElement | undefined;
    if (!el) return;
    const top = el.offsetTop;
    const bottom = top + el.offsetHeight;
    if (top < list.scrollTop) list.scrollTop = top;
    else if (bottom > list.scrollTop + list.clientHeight) list.scrollTop = bottom - list.clientHeight;
  }

  async function openList(dir: 1 | -1 = 1): Promise<void> {
    if (open || !options.length) return;
    active = selected >= 0 ? selected : dir === 1 ? 0 : options.length - 1;
    open = true;
    placed = false;
    bind();
    await tick();
    place();
    placed = true;
    reveal();
  }

  function choose(i: number): void {
    const opt = options[i];
    if (!opt) return;
    dispatch('change', opt.value);
    close(true);
  }

  function move(delta: number): void {
    if (!options.length) return;
    active = (active + delta + options.length) % options.length;
    reveal();
  }

  /** 可打印字符按选项首字母（前缀）循环定位。 */
  function jump(ch: string): void {
    const now = Date.now();
    typed.text = now - typed.at > 700 ? ch : typed.text + ch;
    typed.at = now;
    const q = typed.text.toLowerCase();
    const n = options.length;
    if (!n) return;
    const from = typed.text.length === 1 ? active + 1 : active;
    for (let i = 0; i < n; i++) {
      const idx = (from + i) % n;
      if (labelText(options[idx], $t).toLowerCase().startsWith(q)) {
        active = idx;
        reveal();
        return;
      }
    }
  }

  async function onTriggerKey(e: KeyboardEvent): Promise<void> {
    const k = e.key;
    if (!open) {
      if (k === 'Enter' || k === ' ' || k === 'ArrowDown' || k === 'ArrowUp' || k === 'Home' || k === 'End') {
        if (k !== 'Enter') e.preventDefault();
        e.stopPropagation();
        await openList(k === 'ArrowUp' || k === 'End' ? -1 : 1);
        return;
      }
      if (k.length === 1 && k !== ' ') {
        await openList(1);
        jump(k);
      }
      return;
    }
    // 已展开
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
        // 交给浏览器移走焦点，这里只负责收起。
        close();
        break;
      default:
        if (k.length === 1) {
          e.preventDefault();
          jump(k);
        }
    }
    // 展开期间的按键不该再传给页面上的其它监听（弹窗的 Esc、命令条快捷键等）。
    if (k !== 'Tab') e.stopPropagation();
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
    // 列表自身的滚动（含拖动它自己的滚动条）不算「页面滚动」。
    if (list && e.target instanceof Node && list.contains(e.target)) return;
    close();
  }

  function onResize(): void {
    close();
  }

  /** 焦点若已不在按钮上（例如点在列表空白处），Esc 仍需收得起这块列表。 */
  function onWinKey(e: KeyboardEvent): void {
    if (e.key === 'Escape' && open) {
      e.stopPropagation();
      close(true);
    }
  }

  // 页面切换 / 字段被 showWhen 撤掉时组件会卸载：把挂在 window 上的监听清掉。
  onDestroy(unbind);
</script>

<div class="sel" bind:this={root}>
  <button
    bind:this={trigger}
    class="sel-trigger"
    type="button"
    role="combobox"
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-controls={open ? listId : undefined}
    aria-label={label}
    aria-activedescendant={open ? `${uid}-opt-${active}` : undefined}
    on:click={() => (open ? close() : openList(1))}
    on:keydown={onTriggerKey}
  >
    <span class="sel-text">{currentText}</span>
    <ChevronDown />
  </button>
  {#if open}
    <div
      class="sel-list"
      class:on={placed}
      class:up
      id={listId}
      role="listbox"
      aria-label={label}
      bind:this={list}
      style={maxHeight ? `max-height:${maxHeight}px` : undefined}
    >
      {#each options as o, i (o.value)}
        <button
          class="sel-opt"
          class:active={i === active}
          type="button"
          role="option"
          tabindex="-1"
          id={`${uid}-opt-${i}`}
          aria-selected={i === selected}
          on:mousedown|preventDefault
          on:click={() => choose(i)}
          on:mousemove={() => (active = i)}
        >
          {labelText(o, $t)}
        </button>
      {/each}
    </div>
  {/if}
</div>
