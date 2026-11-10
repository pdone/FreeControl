/**
 * 全部 6 个模态弹窗（添加设备 / 设备参数 / 关于 / 删除设备确认 / 首次启动引导 /
 * 未经官方校验的安装确认）共用的过渡参数。
 *
 * 它们都是「只淡遮罩、面板不缩放」：面板一旦带缩放，顶边就要从下往上归位一段（0.94→1 在
 * 520px 高的面板上是 15.65px），观感上像「弹出来之后内容还在动」。关于弹窗原按 `about-page`
 * 规格单独保留了缩放，2026-10-27 起与其余弹窗对齐。
 *
 * 后续新增模态弹窗沿用这份参数；非模态浮层（hover 提示、帮助面板、下拉菜单）不适用。
 */
export const prefersReducedMotion =
  typeof window !== 'undefined' &&
  !!window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;

export const DIALOG_FADE = { duration: prefersReducedMotion ? 1 : 160 };
