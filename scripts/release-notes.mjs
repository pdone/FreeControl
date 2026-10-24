// 发行正文生成：Update.md / Update.en.md --> 草稿发行的正文（design D1–D3）
//
// 用法：
//   node scripts/release-notes.mjs 1.2.0                 写 stdout
//   node scripts/release-notes.mjs 1.2.0 --out notes.md  写文件
//
// 只认应用版本（如 1.2.0，不带前导 v）：版本号的唯一来源是
// src-tauri/tauri.conf.json，tag 由它拼出，正文因此与 tag 天然对齐。
// 缺该版本的条目时非零退出，并列出两份文档现有的版本标题 ——
// 出包时由调用方决定退回通用说明（产物优先），本地预览则直接看见问题。
//
// 依赖：仅 Node 内置模块。

import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const USAGE = '用法：node scripts/release-notes.mjs <版本> [--out <路径>]';

const DOCS = [
  { file: 'Update.md', heading: '## 中文' },
  { file: 'Update.en.md', heading: '## English' },
];

const VERSION_RE = /^\d+\.\d+\.\d+$/;
const VERSION_HEADING_RE = /^##\s+v/;
const ANY_H2_RE = /^##\s/;

function fail(message) {
  console.error(message);
  process.exit(1);
}

let outPath = null;
const positional = [];
const argv = process.argv.slice(2);
for (let i = 0; i < argv.length; i += 1) {
  const arg = argv[i];
  if (arg === '--out') {
    outPath = argv[i + 1] ?? null;
    if (!outPath) fail(`--out 需要一个路径。\n${USAGE}`);
    i += 1;
  } else if (arg.startsWith('--out=')) {
    outPath = arg.slice('--out='.length);
  } else if (arg === '--help' || arg === '-h') {
    console.log(USAGE);
    process.exit(0);
  } else {
    positional.push(arg);
  }
}

if (positional.length !== 1) fail(USAGE);
const version = positional[0];

if (/^v/i.test(version)) {
  const bare = version.replace(/^v/i, '');
  fail(`版本号不接受前导 "v"：收到 "${version}"，应为 "${bare}"（tag 由应用版本拼出，不在正文生成这边输入）。`);
}
if (!VERSION_RE.test(version)) {
  fail(`版本号必须形如 1.2.0（三段式数字）：收到 "${version}"。`);
}

function versionHeadings(lines) {
  return lines.filter((line) => VERSION_HEADING_RE.test(line)).map((line) => line.trim());
}

// 从 `## v<版本>` 的下一行取到下一个二级标题或文末，去掉首尾空行。
function sectionOf(lines, wanted) {
  const target = `## v${wanted}`;
  const start = lines.findIndex((line) => line.trim() === target);
  if (start < 0) return null;
  let end = lines.length;
  for (let i = start + 1; i < lines.length; i += 1) {
    if (ANY_H2_RE.test(lines[i])) {
      end = i;
      break;
    }
  }
  return lines
    .slice(start + 1, end)
    .join('\n')
    .replace(/^\n+/, '')
    .replace(/\n+$/, '');
}

const blocks = [];
const missing = [];

for (const doc of DOCS) {
  const lines = readFileSync(join(root, doc.file), 'utf8').split(/\r?\n/);
  const body = sectionOf(lines, version);
  if (body == null) {
    missing.push({ doc, headings: versionHeadings(lines) });
  } else {
    blocks.push(`${doc.heading}\n\n${body}`);
  }
}

if (missing.length > 0) {
  for (const { doc, headings } of missing) {
    console.error(`找不到 ${doc.file} 中的 "## v${version}" 一节。`);
    console.error(`  该文件现有的版本标题：${headings.length > 0 ? headings.join('、') : '（无）'}`);
  }
  console.error('两份更新日志须同时含该版本的条目（见 changelog 能力「英文更新日志与中文档成对」）。');
  process.exit(1);
}

const notes = `${blocks.join('\n\n')}\n`;

if (outPath) {
  writeFileSync(outPath, notes, 'utf8');
  console.error(`已写入 ${outPath}（v${version}，中文节 + 英文节）`);
} else {
  process.stdout.write(notes);
}
