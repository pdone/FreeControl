/**
 * 三个弹窗（添加设备 / 设备参数 / 关于）共用的过渡参数。
 *
 * 三者都是「只淡遮罩、面板不缩放」：面板一旦带缩放，顶边就要从下往上归位一段（0.94→1 在
 * 520px 高的面板上是 15.65px），观感上像「弹出来之后内容还在动」。关于弹窗原按 `about-page`
 * 规格单独保留了缩放，2026-10-27 起与另两个对齐。
 */
export const prefersReducedMotion =
  typeof window !== 'undefined' &&
  !!window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;

export const DIALOG_FADE = { duration: prefersReducedMotion ? 1 : 160 };
