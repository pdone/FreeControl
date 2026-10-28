import App from './App.svelte';
import './tokens.css';
import './app.css';
import { loadWebFont } from './lib/web-font';

const app = new App({ target: document.getElementById('app')! });

// 正文字体懒加载（见 lib/web-font.ts）：不占首屏带宽，等页面与交互都就绪后的空闲帧再取。
// requestIdleCallback 在部分引擎不存在，且空闲帧可能长期不来 —— 故都带兜底时限。
const scheduleFont = () => {
  const idle = (window as unknown as { requestIdleCallback?: (cb: () => void, o?: { timeout: number }) => void })
    .requestIdleCallback;
  if (idle) idle(loadWebFont, { timeout: 1500 });
  else window.setTimeout(loadWebFont, 300);
};
if (document.readyState === 'complete') scheduleFont();
else window.addEventListener('load', scheduleFont, { once: true });

export default app;
