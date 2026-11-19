/**
 * 遮罩关闭：仅当按下也发生在遮罩上时才算一次「点遮罩」。
 *
 * 弹窗顶部条可拖动窗口；双击它在最大化 / 还原之间切换时，窗口会在 mouseup 前改变尺寸，
 * 落点可能变成遮罩 —— 不校验按下来源的话，这次双击会被当成点遮罩而把弹窗关掉。
 */
export function backdropClose(node: HTMLElement, handler: () => void) {
  let down = false;
  const onDown = () => {
    down = true;
  };
  const onClick = () => {
    if (!down) return;
    down = false;
    handler();
  };
  node.addEventListener('mousedown', onDown);
  node.addEventListener('click', onClick);
  return {
    update(next: () => void) {
      handler = next;
    },
    destroy() {
      node.removeEventListener('mousedown', onDown);
      node.removeEventListener('click', onClick);
    },
  };
}
