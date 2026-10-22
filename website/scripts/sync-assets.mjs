// 把仓库唯一来源里的品牌资产与原型页面镜像到站点根，供 Vite 直接伺服。
// 与 scripts/sync-icons.mjs 同范式：按清单镜像、先清空再拷贝（换设计后不残留旧文件）。
// 镜像目标（public/assets/、public/preview/）已在 .gitignore 中排除 —— 仓库内不留副本。
import { copyFile, mkdir, rm } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const siteRoot = resolve(here, '..');
const repoRoot = resolve(siteRoot, '..');
const outRoot = resolve(siteRoot, 'public');

// 清单即唯一事实来源：源（相对仓库根）-> 站点内相对 public/ 的目标路径
// 原型页面是 hero 宽屏分支内嵌的界面，设备页截图是 hero 窄屏分支的回退图 ——
// 两者都只有一份来源，站点在构建期镜像它们，不留副本。
const MANIFEST = [
  ['assets/icons/desktop/64x64.png', 'assets/icons/desktop/64x64.png'],
  ['assets/icons/desktop/128x128.png', 'assets/icons/desktop/128x128.png'],
  ['assets/icons/desktop/256x256.png', 'assets/icons/desktop/256x256.png'],
  ['assets/icons/desktop/512x512.png', 'assets/icons/desktop/512x512.png'],
  ['ui-design/launcher-prototype.html', 'preview/launcher-prototype.html'],
  ['assets/screenshots/devices.png', 'screenshots/devices.png'],
  ['assets/screenshots/en/devices.png', 'screenshots/en/devices.png'],
];

// 先清空再拷贝
await rm(outRoot, { recursive: true, force: true });

for (const [src, dest] of MANIFEST) {
  const from = resolve(repoRoot, src);
  const to = resolve(siteRoot, 'public', dest);
  await mkdir(dirname(to), { recursive: true });
  await copyFile(from, to);
}

console.log(`[sync-assets] mirrored ${MANIFEST.length} files into public/`);
