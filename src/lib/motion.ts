/**
 * 全部 6 个模态弹窗（添加设备 / 设备参数 / 关于 / 删除设备确认 / 首次启动引导 /
 * 未经官方校验的安装确认）共用的过渡参数。
 *
 * 决策链（2026-11-12 定稿 B）：2026-10-27 曾因「弹出来之后内容还在动」去掉面板缩放；
 * 2026-11-12 用户要求恢复缩放开合，先做 B 版原型（整面板缩放），评审一度退回 A（壳缩放、
 * 内容钉死）并做出 A 版原型；两版对比后用户定稿 B，规则：**位移只允许发生在窗口变大的
 * 过渡播放期间，播完后文字不再位移**。这由 transform 的构造保证——缩放不影响布局，
 * 过渡结束于 scale(1)，Svelte 播完即移除内联样式，零残留（spec `app-shell`「位移仅限过渡期间」）。
 *
 * 面板缩放（dialogScale）只动画 transform；opacity 由遮罩淡入（DIALOG_FADE）单一提供，
 * 避免面板被两层过渡叠乘成双重淡入。两者共用时长与缓动（均 cubicOut），入场 / 退场同步反转。
 * 遮罩淡入曾缺省 linear，比 cubicOut 的面板缩放「到位」慢半拍（80ms 时面板已 99%、遮罩才 52%），
 * 观感上像面板先弹出、遮罩随后才压暗；2026-11-13 起淡入也取 cubicOut，与面板同步到位
 * （原型两侧本就同用 ease，无此错配）。
 *
 * 后续新增模态弹窗沿用这份参数；非模态浮层（hover 提示、帮助面板、下拉菜单）不适用。
 */
import { cubicOut } from 'svelte/easing';
import type { TransitionConfig } from 'svelte/transition';

export const prefersReducedMotion =
  typeof window !== 'undefined' &&
  !!window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;

export const DIALOG_FADE = { duration: prefersReducedMotion ? 1 : 160, easing: cubicOut };

export const DIALOG_SCALE = { duration: prefersReducedMotion ? 1 : 160, start: 0.94 };

/** 面板缩放：transform-only，原点为面板中心（transform 默认 origin）。 */
export function dialogScale(
  _node: Element,
  params: { duration?: number; start?: number } = DIALOG_SCALE,
): TransitionConfig {
  const duration = params.duration ?? DIALOG_SCALE.duration;
  const start = params.start ?? DIALOG_SCALE.start;
  return {
    duration,
    easing: cubicOut,
    css: (t) => `transform: scale(${start + (1 - start) * t})`,
  };
}