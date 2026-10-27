import './app.css';
import { initLanguage } from './lib/i18n';
import { initFontStacks } from './lib/fonts';
import { applyUiScaleFromCache } from './lib/scale';
import App from './App.svelte';
import { revealWindowWhenPainted } from './lib/store';

// 挂载前先把首帧语言写到根元素：配置是异步读取的，这里用本机上次生效的语言
// （没有快照时按系统语言）兜住配置到达前的第一帧；配置到达后由 store 的订阅覆盖。
initLanguage();

// 字体同理由 localStorage 的首帧缓存兜住：配置到达后由 store 的订阅换成盘上的顺序，
// 避免窗口已经显示、配置随后到达时肉眼可见地换一次字体。
initFontStacks();

// 界面缩放与上面两件不同：它是 WebView 的缩放因子，不是能同步写进首帧的 DOM 样式。
// 这里与外壳渲染**并行**发起（不 await，免得挡住首帧），再由 `revealWindowWhenPainted`
// 在 `show()` 之前等它（150ms 上限）—— 否则首次上屏会先以 100% 出现再跳一次。
const scaleReady = applyUiScaleFromCache();

const app = new App({
  target: document.getElementById('app')!,
});

// 主窗口以隐藏方式创建（`src-tauri/tauri.conf.json` 的 `visible: false`），到这里才准备放它出来。
// Svelte 4 的 `new App(...)` 是**同步渲染**，上面这行返回时外壳已在 DOM 中；但要等它**真的画出来**
// 再显示窗口，否则那几帧里用户看到的是一块纯色 —— 时机的判断在 `revealWindowWhenPainted` 里，
// 连同"为什么不能等固定帧数"的实测依据一起（tasks 1.1 / 5.1）。
void revealWindowWhenPainted(scaleReady);

export default app;
