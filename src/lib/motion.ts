/**
 * 添加设备与设备参数弹窗共用的过渡参数。
 *
 * 两个弹窗都是「工作弹窗」：出现与消失只淡遮罩、面板不缩放。关于弹窗是信息弹窗，
 * 按 `about-page` 规格保留自己的缩放动画，不引用这里。
 */
export const prefersReducedMotion =
  typeof window !== 'undefined' &&
  !!window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;

export const DIALOG_FADE = { duration: prefersReducedMotion ? 1 : 160 };
