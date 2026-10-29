/**
 * 生成官网正文字体子集（阿里巴巴普惠体 3.0 · 55 Regular → woff2）。
 *
 * 为什么要有这个脚本：普惠体全量 TTF 8 MB，直接上线等于给首屏加一次大下载；
 * 官网是单页且文案固定，按"页面实际出现的字"取子集后只有 ~76 KiB（woff2）。
 * 子集是**生成物入库**（`src/assets/fonts/`），构建不依赖 Python —— 与图标子集
 * （`scripts/gen-icon-subset.mjs`）同一套习惯：生成器入库、产物入库、构建只用产物。
 *
 * 用法（需要 Python + fonttools + brotli）：
 *   node scripts/gen-font-subset.mjs --font "<…55-Regular.ttf>" --weight 400
 * 未给 --font 时读环境变量 PUHUITI_TTF；未给 --weight 时按 400（也是唯一支持的档）。
 * 产物名固定为 `puhuiti3-55-subset.woff2`，与 `web-font.ts` 里那条 `@font-face` 一一对应 ——
 * 改了命名规则要同时改那边。
 *
 * 何时需要重跑：改了页面文案、且新文案里出现了旧子集没有的字。不重跑也不会出现豆腐块 ——
 * 缺字由 `--font-sans-system` 这条系统栈逐字兜底，只是那几个字与其余部分不同体。
 *
 * 字体来源与授权：阿里巴巴普惠体 3.0（Alibaba PuHuiTi 3.0），阿里官方免费商用字体，
 * 允许商用与再分发，MUST NOT 单独售卖字体本身。取用前请按官方页面（fonts.alibabagroup.com）
 * 的条款自行确认；本仓库只入库"按本页用字裁出的子集"，不是字体发行物。
 */
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
/** 取字的来源文件：站点所有可见文案都在这几处（含内嵌预览的 alt/标题）。 */
const SOURCES = ['src/lib/i18n.ts', 'src/App.svelte', 'index.html'];

const argFont = process.argv.indexOf('--font');
const fontPath = argFont > -1 ? process.argv[argFont + 1] : process.env.PUHTUITI_TTF || process.env.PUHUITI_TTF;
if (!fontPath || !fs.existsSync(fontPath)) {
  console.error('[gen-font-subset] 找不到字体文件。用法：node scripts/gen-font-subset.mjs --font "<*.ttf 路径>" --weight 400');
  process.exit(1);
}

// 字重档 → 产物名：站点只带一份 55 Regular。标题的 600–900 由浏览器在这一份轮廓上合成加粗
// （见 web-font.ts 的 `font-weight: 400` 与 spec「正文字体只提供单一字重档」）。
const WEIGHT_TO_FILE = { 400: 'puhuiti3-55-subset.woff2' };
const argWeight = process.argv.indexOf('--weight');
const weight = argWeight > -1 ? Number(process.argv[argWeight + 1]) : 400;
if (!WEIGHT_TO_FILE[weight]) {
  console.error(`[gen-font-subset] 不支持的字重档 ${weight}；只接受 ${Object.keys(WEIGHT_TO_FILE).join(' / ')}`);
  process.exit(1);
}
const OUT = path.join(ROOT, 'src/assets/fonts', WEIGHT_TO_FILE[weight]);

// 1. 收集字符：源文件里的全部字符 + 全 ASCII + 常用中文标点（防止标点回退成系统体）
const chars = new Set();
for (const rel of SOURCES) {
  const file = path.join(ROOT, rel);
  if (!fs.existsSync(file)) continue;
  for (const ch of fs.readFileSync(file, 'utf8')) {
    const cp = ch.codePointAt(0);
    if (cp > 0x1f && cp !== 0x7f) chars.add(ch);
  }
}
for (let cp = 0x20; cp < 0x7f; cp++) chars.add(String.fromCodePoint(cp));
for (const ch of '，。、；：？！“”‘’（）《》〈〉【】—…·／％＋－×÷≈≤≥°') chars.add(ch);

// 2. 调 fonttools 做子集（--layout-features 清空：单页展示用不到 kerning/liga 之外的排版特性）
const tmp = path.join(os.tmpdir(), `fc-font-subset-${Date.now()}.txt`);
fs.writeFileSync(tmp, Array.from(chars).join(''), 'utf8');
fs.mkdirSync(path.dirname(OUT), { recursive: true });
const run = spawnSync(
  'python',
  [
    '-m', 'fontTools.subset', fontPath,
    `--text-file=${tmp}`,
    '--flavor=woff2',
    `--output-file=${OUT}`,
    '--layout-features=', // 只保留默认特性，去掉 ss01/cv* 这类展示型开关
    '--no-hinting',
    '--desubroutinize',
    '--drop-tables+=DSIG',
  ],
  { stdio: 'inherit' },
);
fs.rmSync(tmp, { force: true });
if (run.status !== 0) {
  console.error('[gen-font-subset] fonttools 失败；确认已 `pip install fonttools brotli`。');
  process.exit(run.status ?? 1);
}

const bytes = fs.statSync(OUT).size;
console.log(
  `[gen-font-subset] 字重档 ${weight}：${chars.size} 个字符 → ${path.relative(ROOT, OUT)}（${(bytes / 1024).toFixed(1)} KB）`,
);
