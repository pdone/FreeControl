<script lang="ts">
  import { onMount } from 'svelte';
  import { activeTip, TIP_LAYER_ID, type ActiveTip } from '../lib/tooltip';

  let panel: HTMLElement | null = null;
  /** 面板测得尺寸前的首帧先不显形：否则会在上一个提示的坐标上闪一下、动画也从错处开跑。 */
  let placed = false;
  let x = 0;
  let y = 0;
  /** 面板落在触发元素的哪一侧（`bottom` = 面板在控件下方）。 */
  let dir: 'top' | 'bottom' | 'left' | 'right' = 'bottom';
  /** 箭头沿其所在边的偏移（`--tip-shift`）。 */
  let shift = 12;

  /** 视口边距、控件与提示之间的缝。 */
  const M = 8;
  const GAP = 8;

  /** 箭头贴哪条边 = 面板所在侧的反面（面板在下方 ⇒ 箭头在上边，朝上指）。 */
  const EDGE = { bottom: 'top', top: 'bottom', left: 'right', right: 'left' } as const;
  type Side = keyof typeof EDGE;

  /**
   * rAF 句柄放在对象里而不是 `let`：reactive 语句里读一个 `let` 会把它算成依赖，
   * 而循环每帧都要写它 —— 那会让那条语句反复重入。
   */
  const loop = { id: 0 };
  /** 上一次显示的触发元素：换了它才重来一次「先隐形再落位」，同一次悬停内改文本不该再弹一下。 */
  let lastAnchor: HTMLElement | null = null;

  function clamp(v: number, lo: number, hi: number): number {
    return Math.min(Math.max(v, lo), Math.max(lo, hi));
  }

  /**
   * 量一次面板，**贴着触发元素**择侧落位，再夹进视口；箭头落在朝向控件的那条边上、
   * 沿边对准控件的中心 —— 提示因此永远指得住控件，而不是指向指针或空白。
   */
  function place(t: ActiveTip): void {
    const el = panel;
    if (!el) return;
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const a = t.anchor.getBoundingClientRect();
    // 各侧的可用空间（已扣掉缝与视口边距）：放得下整块面板才算「可落」。
    const room: Record<Side, number> = {
      bottom: innerHeight - a.bottom - GAP - M,
      top: a.top - GAP - M,
      right: innerWidth - a.right - GAP - M,
      left: a.left - GAP - M,
    };
    const need: Record<Side, number> = { bottom: h, top: h, right: w, left: w };
    // 候选顺序：控件自己声明的偏好在前（侧栏收起态用 `right`），其余按 下 → 上 → 右 → 左。
    const order: Side[] =
      t.placement === 'right' ? ['right', 'bottom', 'top', 'left'] : ['bottom', 'top', 'right', 'left'];
    // 都放不下时挑最宽裕的一侧，交给下面的夹紧兜底。
    dir =
      order.find((s) => room[s] >= need[s]) ??
      order.reduce((best, s) => (room[s] > room[best] ? s : best), order[0]);

    const acx = a.left + a.width / 2;
    const acy = a.top + a.height / 2;
    if (dir === 'bottom' || dir === 'top') {
      // 面板在控件上/下：横向居中于控件；箭头沿上/下边，对准控件的横向中心。
      x = clamp(acx - w / 2, M, innerWidth - M - w);
      y = clamp(dir === 'bottom' ? a.bottom + GAP : a.top - GAP - h, M, innerHeight - M - h);
      shift = Math.round(clamp(acx - x - 4.5, 10, Math.max(10, w - 20)));
    } else {
      // 面板在控件左/右：纵向居中于控件；箭头沿左/右边，对准控件的纵向中心。
      x = clamp(dir === 'right' ? a.right + GAP : a.left - GAP - w, M, innerWidth - M - w);
      y = clamp(acy - h / 2, M, innerHeight - M - h);
      shift = Math.round(clamp(acy - y - 4.5, 10, Math.max(10, h - 20)));
    }
    placed = true;
  }

  function frame(t: ActiveTip): void {
    if (!panel) {
      // 面板要等这一帧的 DOM 更新挂上；还没挂上就再等一帧。
      loop.id = requestAnimationFrame(() => frame(t));
      return;
    }
    // 逐帧跟锚点：滚动、窗口缩放、侧栏收起、列表重排都会改它的位置，其中一些根本没有
    // scroll / resize 事件可接。锚点消失或整个移出视口即收起。
    const a = t.anchor.getBoundingClientRect();
    const gone = !t.anchor.isConnected || a.bottom < 0 || a.top > innerHeight || a.right < 0 || a.left > innerWidth;
    if (gone) {
      activeTip.set(null);
      return;
    }
    place(t);
    loop.id = requestAnimationFrame(() => frame(t));
  }

  function stop(): void {
    if (loop.id) cancelAnimationFrame(loop.id);
    loop.id = 0;
  }

  function start(t: ActiveTip): void {
    stop();
    if (t.anchor !== lastAnchor) {
      lastAnchor = t.anchor;
      placed = false;
    }
    loop.id = requestAnimationFrame(() => frame(t));
  }

  // 只依赖 `$activeTip`：start / stop 的函数体不参与这条语句的依赖分析，
  // 所以 `panel` 绑定时不会把语句反复触发。
  $: if ($activeTip) {
    start($activeTip);
  } else {
    stop();
    lastAnchor = null; // 收起了：下次显示重新量一次、重新播一遍入场
  }

  onMount(() => {
    const hide = (): void => activeTip.set(null);
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') hide();
    };
    window.addEventListener('keydown', onKey);
    // 指针按下即收起：点下去之后控件多半要开弹窗 / 换页，提示不该继续挂着。
    window.addEventListener('pointerdown', hide, true);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('pointerdown', hide, true);
      stop();
    };
  });
</script>

<!-- 提示层常驻 DOM（id 供触发元素的 `aria-describedby` 指过来），无提示时内部不渲染面板。
     层自身以视口定位、不参与任何布局：它是应用级的 fixed 子项，不受 `.app` 的 overflow 影响。 -->
<div class="tip-layer" id={TIP_LAYER_ID}>
  {#if $activeTip}
    <span
      class="tip-panel side-{EDGE[dir]}"
      class:on={placed}
      role="tooltip"
      bind:this={panel}
      style="left:{x}px; top:{y}px; --tip-shift:{shift}px">{$activeTip.text}</span
    >
  {/if}
</div>
