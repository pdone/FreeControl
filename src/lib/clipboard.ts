// 复制纯文本到系统剪贴板（tasks 8.4）。
//
// 不引入剪贴板插件，直接用 WebView 自身的能力 —— 复制是纯文本写入，Web API 足够：
//   1. `navigator.clipboard.writeText`（主路径）。Tauri 的页面源是 `http://tauri.localhost`，
//      属于可信来源，`isSecureContext` 为真，故该 API 可用；但个别 WebView 会拒绝剪贴板权限。
//   2. `document.execCommand('copy')`（回退）。已废弃，却在用户手势中仍然可用（原型即此做法）。
//
// 失败时**抛出**：调用方据此不给成功反馈，绝不静默「假装已复制」。

export async function copyText(text: string): Promise<void> {
  const clip = navigator.clipboard;
  if (clip && window.isSecureContext) {
    try {
      await clip.writeText(text);
      return;
    } catch {
      // 落到下面的回退路径。
    }
  }

  // 回退：临时 textarea + 选中 + execCommand。位置放到视口外，避免闪一下选中高亮。
  const ta = document.createElement('textarea');
  ta.value = text;
  ta.setAttribute('readonly', '');
  ta.style.position = 'fixed';
  ta.style.top = '-1000px';
  document.body.appendChild(ta);
  ta.select();
  ta.setSelectionRange(0, ta.value.length);
  try {
    // 抛消息对象而不是成品字符串：复制失败提示要随界面语言重译（见 `message.ts`）。
    if (!document.execCommand('copy')) throw { code: 'clipboard.unavailable' };
  } finally {
    document.body.removeChild(ta);
  }
}
