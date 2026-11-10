/**
 * 关闭主窗口时的唯一决策：隐藏到托盘，还是真正退出。
 *
 * 三处判据缺一不可：
 * - `quitting`：托盘「退出」或退出流程已开始 —— 即使偏好是收进托盘也必须退出；
 * - `closeToTray`：用户偏好；
 * - `trayReady`：托盘真的存在。创建失败时不能把窗口藏到无处找回。
 *
 * 纯函数：关闭钩子、托盘退出与单测共用同一份判定，避免两处判据分叉。
 */
export type CloseAction = 'hide' | 'quit';

export interface CloseDecisionInput {
  closeToTray: boolean;
  trayReady: boolean;
  quitting: boolean;
}

export function decideCloseAction(input: CloseDecisionInput): CloseAction {
  if (input.quitting) return 'quit';
  return input.closeToTray && input.trayReady ? 'hide' : 'quit';
}
