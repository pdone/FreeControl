import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  // 站点自己的 JS/CSS 产物落在 dist/bundle/，把 dist/assets/ 让给镜像进来的品牌资产与截图
  build: { assetsDir: 'bundle' },
});
