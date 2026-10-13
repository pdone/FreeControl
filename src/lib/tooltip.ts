import { writable, get } from 'svelte/store';

/**
 * hover 提示的落位偏好：默认落在触发元素**下方**居中；侧边栏收起态那类窄栏用 `right`，
 * 让提示落到栏外（提示层在文档顶层，不受 `.sidebar` 的 overflow 裁剪）。
 */
export type TipPlacement = 'bottom' | 'right';

export type ActiveTip = {
  text: string;
  anchor: HTMLElement;
  placement: TipPlacement;
};

/** 提示层的 id：容器常驻 DOM（见 `TooltipLayer.svelte`），触发元素经 `aria-describedby` 指它。 */
export const TIP_LAYER_ID = 'app-tip-layer';

/** 指针悬停多久才显示提示：扫过一排控件不会逐个闪，需要看提示时停住一拍就到。 */
export const TIP_SHOW_DELAY = 400;

/** 单例提示状态：同一时刻最多一条提示，由顶层提示层渲染。 */
export const activeTip = writable<ActiveTip | null>(null);

export type TipOptions = string | { text: string; placement?: TipPlacement } | null | undefined;

function read(options: TipOptions): { text: string; placement: TipPlacement } | null {
  if (options == null) return null;
  const text = typeof options === 'string' ? options : options.text;
  // 文本为空即「这条控件没有提示」：侧边栏展开态就是靠传空，把收起态那条提示摘掉的。
  if (!text) return null;
  const placement = typeof options === 'string' ? 'bottom' : options.placement ?? 'bottom';
  return { text, placement };
}

/**
 * 可聚焦元素：`aria-describedby` 要落在**真正会被聚焦**的那个元素上 —— 禁用控件外面的承载层
 * （`span.tip-host`）与 `.switch` 那类 `label` 都不是焦点所在的元素。
 */
const FOCUSABLE = 'button, a[href], input, select, textarea, [tabindex]';

function describeTargets(node: HTMLElement): HTMLElement[] {
  if (node.matches(FOCUSABLE)) return [node];
  const inner = node.querySelector<HTMLElement>(FOCUSABLE);
  return inner ? [inner] : [];
}

/**
 * 把提示接到触发元素上（`use:tip={文本}` 或 `use:tip={{ text, placement }}`）。
 *
 * 落位一律**贴着触发元素**：指针触发要停住 `TIP_SHOW_DELAY` 才显示，键盘聚焦立即显示；
 * 两者都离开才收起 —— 与帮助面板 `.fhelp` 的 `:hover` / `:focus-within` 同一个口径。
 * 聚焦只在 `:focus-visible` 时算数：鼠标点击留下的那个焦点若也算，点完按钮把指针移开、
 * 提示会一直挂着不走。
 */
export function tip(node: HTMLElement, options?: TipOptions) {
  let current = read(options);
  let hovered = false;
  let focusVisible = false;
  /** 本次悬停是否已到点（延迟满了）。未到点前只挂计时器，不显示。 */
  let revealed = false;
  let timer = 0;
  const described = describeTargets(node);

  const hideOwn = (): void => {
    // 只摘自己那条：别的触发元素重算（如文本随状态变化）时不许把别人正在显示的提示清掉。
    if (get(activeTip)?.anchor === node) activeTip.set(null);
  };
  const cancelTimer = (): void => {
    if (timer) {
      clearTimeout(timer);
      timer = 0;
    }
  };

  /** 按当前状态把该显示的显示出来。不改计时器（计时器只由 enter / leave 驱动）。 */
  const sync = (): void => {
    if (current === null || (!hovered && !focusVisible)) {
      hideOwn();
      return;
    }
    if (focusVisible || revealed) {
      // 已经是自己那条（如文本随状态变化）就不重设，免得提示层的落位重来一遍。
      if (get(activeTip)?.anchor !== node) activeTip.set({ ...current, anchor: node });
    }
  };

  const onEnter = (): void => {
    hovered = true;
    revealed = false;
    cancelTimer();
    timer = window.setTimeout(() => {
      timer = 0;
      revealed = true;
      sync();
    }, TIP_SHOW_DELAY);
    sync();
  };
  const onLeave = (): void => {
    hovered = false;
    revealed = false;
    cancelTimer();
    sync();
  };
  const onFocusIn = (): void => {
    focusVisible = node.matches(':focus-visible') || described.some((el) => el.matches(':focus-visible'));
    sync();
  };
  const onFocusOut = (e: FocusEvent): void => {
    // 焦点在节点内部挪动（如按钮里的子元素）不算离开。
    if (e.relatedTarget instanceof Node && node.contains(e.relatedTarget)) return;
    focusVisible = false;
    sync();
  };

  node.addEventListener('pointerenter', onEnter);
  node.addEventListener('pointerleave', onLeave);
  node.addEventListener('focusin', onFocusIn);
  node.addEventListener('focusout', onFocusOut);
  // 自绘提示取代了原生 title，可访问性描述改由提示层承担（提示层挂载时才有文本）。
  for (const el of described) {
    if (!el.hasAttribute('aria-describedby')) el.setAttribute('aria-describedby', TIP_LAYER_ID);
  }

  return {
    update(next?: TipOptions): void {
      current = read(next);
      sync();
    },
    destroy(): void {
      node.removeEventListener('pointerenter', onEnter);
      node.removeEventListener('pointerleave', onLeave);
      node.removeEventListener('focusin', onFocusIn);
      node.removeEventListener('focusout', onFocusOut);
      cancelTimer();
      for (const el of described) {
        if (el.getAttribute('aria-describedby') === TIP_LAYER_ID) el.removeAttribute('aria-describedby');
      }
      // 节点被移除（设备行消失、面板收起…）时把自己那条收掉，免得提示层还按旧锚点定位。
      hideOwn();
    },
  };
}
