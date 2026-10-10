// 桌面图标同步：assets/icons/desktop/ --> src-tauri/icons/  （design D8）
//
// 唯一资产来源：assets/icons/ 资产包。其中 desktop/ 是**已产出的成品**图标
// （icon.ico 含 16–256 共 7 层、icon.icns 含 16–1024 共 11 层、各尺寸 PNG 均为原生生成），
// 由包内 build_icons.py 从 1024 母版产出 —— 本项目不再自行组装 ICO/ICNS。
//
// 同步清单的唯一事实来源是 src-tauri/tauri.conf.json 的 bundle.icon：
// 本脚本不硬编码文件名，改平台支持范围只改那份清单。
//
// 同步方式是**镜像**（先清空目标目录再拷贝）而不是叠加拷贝 ——
// 更换设计或精简清单后，旧图标必须消失，否则会残留成「看起来没生效」。
//
// 依赖：仅 Node 内置模块。
// 用法：node scripts/sync-icons.mjs

import { readFileSync, mkdirSync, rmSync, copyFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, basename } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const srcDir = join(root, 'assets', 'icons', 'desktop');
const outDir = join(root, 'src-tauri', 'icons');
const confPath = join(root, 'src-tauri', 'tauri.conf.json');

if (!existsSync(srcDir)) {
  throw new Error(`找不到资产包目录 ${srcDir}`);
}

const conf = JSON.parse(readFileSync(confPath, 'utf8'));
const declared = conf?.bundle?.icon;
if (!Array.isArray(declared) || declared.length === 0) {
  throw new Error('tauri.conf.json 的 bundle.icon 为空，无法确定要同步哪些图标');
}

// bundle.icon 里的路径相对 src-tauri/，形如 "icons/32x32.png"
const wanted = declared.map((p) => {
  const norm = String(p).replace(/\\/g, '/');
  if (!norm.startsWith('icons/')) {
    throw new Error(`bundle.icon 路径必须位于 icons/ 下：${p}`);
  }
  return basename(norm);
});

// 镜像同步：先清空，避免换设计后旧图标残留
rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });

const missing = [];
for (const name of wanted) {
  const src = join(srcDir, name);
  if (!existsSync(src)) {
    missing.push(name);
    continue;
  }
  copyFileSync(src, join(outDir, name));
}
if (missing.length > 0) {
  throw new Error(
    `资产包缺少 bundle.icon 声明的文件：${missing.join(', ')}（查找目录 ${srcDir}）`,
  );
}

console.log(`[sync-icons] assets/icons/desktop -> src-tauri/icons：${wanted.join(', ')}`);
