import App from './App.svelte';
import './tokens.css';
import './app.css';
import { installWebFont } from './lib/web-font';

// 正文字体在第一帧之前声明（见 lib/web-font.ts）：站点是纯客户端渲染，正文的第一个字符必然
// 出现在这之后，所以这是能抓到的最早时机。以前把它挂在 `load` 之后的空闲帧上，于是每次刷新
// 都能看到"先系统字体、再整体换一次"；现在缓存命中时首帧直接就是普惠体。
installWebFont();

const app = new App({ target: document.getElementById('app')! });
