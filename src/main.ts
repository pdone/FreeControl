import './app.css';
import App from './App.svelte';
import { revealWindowWhenPainted } from './lib/store';

const app = new App({
  target: document.getElementById('app')!,
});

// 主窗口以隐藏方式创建（`src-tauri/tauri.conf.json` 的 `visible: false`），到这里才准备放它出来。
// Svelte 4 的 `new App(...)` 是**同步渲染**，上面这行返回时外壳已在 DOM 中；但要等它**真的画出来**
// 再显示窗口，否则那几帧里用户看到的是一块纯色 —— 时机的判断在 `revealWindowWhenPainted` 里，
// 连同"为什么不能等固定帧数"的实测依据一起（tasks 1.1 / 5.1）。
void revealWindowWhenPainted();

export default app;
