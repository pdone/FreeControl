// 单选分段控件（`.seg`）的选中指示块（spec `app-shell`「分段控件的选中切换动画」）。
//
// 指示块由 action 创建：容器内一块绝对定位的 accent 实底。用户点击 / 键盘激活切换选项时，
// 实底滑到新选项并按新宽高伸缩；挂载、配置到达、语言切换引起的重排、窗口 / 字体变化与
// 程序化改选一律直接到位，不播动画；主题组也与其它组一致，滑动与整页过渡同时发生。
import { prefersReducedMotion } from './motion';

/** 用户激活后多久内到达的选中变化算「用户点击」：DOM 的 class 更新在同一任务或紧随的微任务里完成。 */
const INTENT_WINDOW = 250;

/** 几何过渡时长（与弹窗淡入同档）；动画结束后再做一次静默校正，接住动画期间的重排。 */
const SLIDE_MS = 160;

function activeButton(node: HTMLElement): HTMLButtonElement | null {
  return node.querySelector<HTMLButtonElement>('button.active');
}

export function segSlide(node: HTMLElement) {
  const indicator = document.createElement('span');
  indicator.className = 'seg-indicator';
  indicator.setAttribute('aria-hidden', 'true');
  node.prepend(indicator);

  /** 上一次落位对应的活动按钮；null 表示还没落过位（首帧不播动画）。 */
  let placedOn: HTMLButtonElement | null = null;
  /** 最近一次用户激活的时间戳。 */
  let intentAt = -Infinity;
  /** 动画结束前忽略重排，避免语言切换引起的按钮重排把正在播放的滑动打断成瞬跳。 */
  let slidingUntil = 0;
  let settleTimer = 0;
  let placeToken = 0;

  function place(animate: boolean): void {
    const btn = activeButton(node);
    placeToken++;
    if (!btn) {
      indicator.style.opacity = '0';
      placedOn = null;
      return;
    }

    const shouldAnimate =
      animate && !prefersReducedMotion && placedOn !== null && btn !== placedOn;

    // 禁用态的按钮靠 CSS opacity .5 变淡；指示块没有自己的 CSS 变淡规则，
    // 统一由行内 opacity 表达（0.5 / 1），避免被行内值盖掉。
    indicator.style.opacity = btn.disabled ? '0.5' : '1';
    if (shouldAnimate) {
      // 上一次落位若关过过渡，先把它恢复并冲掉一帧，否则这次变化的「变化前样式」没有
      // transition-property，浏览器不会启动过渡。
      if (indicator.classList.contains('no-anim')) {
        indicator.classList.remove('no-anim');
        void indicator.offsetWidth;
      }
    } else {
      indicator.classList.add('no-anim');
    }
    indicator.style.transform = `translate(${btn.offsetLeft}px, ${btn.offsetTop}px)`;
    indicator.style.width = `${btn.offsetWidth}px`;
    indicator.style.height = `${btn.offsetHeight}px`;
    placedOn = btn;

    if (shouldAnimate) {
      slidingUntil = performance.now() + SLIDE_MS;
      window.clearTimeout(settleTimer);
      settleTimer = window.setTimeout(() => {
        settleTimer = 0;
        slidingUntil = 0;
        place(false);
      }, SLIDE_MS + 40);
    } else {
      // 放弃过渡的这一次落位要等这帧画完再恢复过渡，否则下一帧可能从更早的几何补间。
      const token = placeToken;
      requestAnimationFrame(() => {
        if (token === placeToken) indicator.classList.remove('no-anim');
      });
    }
  }

  /** 活动项变化才可能播动画；活动项没变（重排、禁用态变化等）只重放几何。 */
  function syncFromActive(): void {
    const btn = activeButton(node);
    if (btn !== placedOn) {
      const animate = performance.now() - intentAt < INTENT_WINDOW;
      intentAt = -Infinity;
      place(animate);
    } else {
      // 活动项没变：重排 / 禁用态变化只重放几何。若滑动还在进行，留给结束后的静默校正，
      // 避免同一次点击的后续 class 变化把动画打断成瞬跳。
      if (performance.now() < slidingUntil) return;
      place(false);
    }
  }

  let observed: HTMLButtonElement[] = [];
  function observeButtons(): void {
    const next = Array.from(node.querySelectorAll<HTMLButtonElement>('button'));
    for (const b of observed) if (!next.includes(b)) resize.unobserve(b);
    for (const b of next) if (!observed.includes(b)) resize.observe(b);
    observed = next;
  }

  const mutation = new MutationObserver((records) => {
    let relevant = false;
    for (const r of records) {
      if (r.type === 'childList') {
        relevant = true;
        break;
      }
      // 指示块自己的 class 变化不算；只看按钮。
      if (r.type === 'attributes' && r.target instanceof HTMLButtonElement) {
        relevant = true;
        break;
      }
    }
    if (!relevant) return;
    observeButtons();
    syncFromActive();
  });
  mutation.observe(node, {
    attributes: true,
    attributeFilter: ['class', 'disabled'],
    childList: true,
    subtree: true,
  });

  const resize = new ResizeObserver(() => {
    if (performance.now() < slidingUntil) return;
    place(false);
  });
  resize.observe(node);
  observeButtons();
  place(false);

  const onIntent = (e: Event): void => {
    // 只认落在按钮上的激活；容器空白处的点击不构成切换意图。
    if (e.target instanceof Element && e.target.closest('button')) {
      intentAt = performance.now();
    }
  };
  node.addEventListener('pointerdown', onIntent);
  node.addEventListener('click', onIntent);
  node.addEventListener('keydown', onIntent);

  return {
    destroy() {
      mutation.disconnect();
      resize.disconnect();
      window.clearTimeout(settleTimer);
      node.removeEventListener('pointerdown', onIntent);
      node.removeEventListener('click', onIntent);
      node.removeEventListener('keydown', onIntent);
      indicator.remove();
    },
  };
}
