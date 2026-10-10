import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Tauri expects a fixed dev server port and ignores Vite's clearScreen.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: false,
    // cargo 的构建产物落在项目根内（`src-tauri/target` 已有 12 GB / 5500 多个目录），
    // 而 Vite 默认只忽略 `.git` / `node_modules` / `test-results`，于是每次启动都要把它
    // 递归遍历一遍去注册文件监听。首个请求（正是 Tauri 打开窗口时发的那一次）会被拖住
    // 数秒，表现为先白屏再出页面 —— 只排除构建产物，Rust 源码仍在监听范围内。
    watch: { ignored: ['**/src-tauri/target/**'] },
  },
  build: {
    target: 'esnext',
  },
});
