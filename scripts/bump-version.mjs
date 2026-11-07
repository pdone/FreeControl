// 应用版本升版：一次改齐六处版本字段与原型两个常量。
//
// 用法：
//   node scripts/bump-version.mjs 2.4.1
//   npm run version:bump -- 2.4.1
//
// 只改版本取值，不写更新日志：版本段的条目要人来写，缺失由
// scripts/check-version.mjs（npm run check:version）拦截。
//
// 依赖：仅 Node 内置模块。

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const USAGE = '用法：node scripts/bump-version.mjs <主.次.修订>\n  例：npm run version:bump -- 2.4.1';
const VERSION_RE = /^(\d+)\.(\d+)\.(\d+)$/;

function fail(message) {
  console.error(message);
  process.exit(1);
}

function parseVersion(value) {
  const m = VERSION_RE.exec(value);
  return m ? m.slice(1).map(Number) : null;
}

function compareVersion(a, b) {
  for (let i = 0; i < 3; i += 1) {
    if (a[i] !== b[i]) return a[i] - b[i];
  }
  return 0;
}

function nextPatch(version) {
  return `${version[0]}.${version[1]}.${version[2] + 1}`;
}

const argv = process.argv.slice(2);
if (argv.length === 1 && (argv[0] === '--help' || argv[0] === '-h')) {
  console.log(USAGE);
  process.exit(0);
}
if (argv.length !== 1) fail(USAGE);

const target = argv[0];
if (/^v/i.test(target)) {
  fail(`版本号不接受前导 "v"：收到 "${target}"，应为 "${target.replace(/^v/i, '')}"。\n${USAGE}`);
}
const targetParts = parseVersion(target);
if (!targetParts) fail(`版本号必须形如 1.2.0（三段式数字）：收到 "${target}"。\n${USAGE}`);

function read(relPath) {
  try {
    return readFileSync(join(root, relPath), 'utf8');
  } catch (error) {
    fail(`读取 ${relPath} 失败：${error.message}`);
  }
}

const tauriText = read('src-tauri/tauri.conf.json');
const currentMatch = /^[ \t]*"version":[ \t]*"([^"]*)"/m.exec(tauriText);
if (!currentMatch) fail('src-tauri/tauri.conf.json 中找不到顶层 "version" 字段。');
const current = currentMatch[1];
const currentParts = parseVersion(current);
if (!currentParts) fail(`当前版本不是三段式数字：src-tauri/tauri.conf.json 为 "${current}"。`);
if (compareVersion(targetParts, currentParts) <= 0) {
  fail(`新版本必须高于当前版本 ${current}：收到 "${target}"。`);
}

const replacements = [];

// 按顶层 `"version": "..."` 出现的顺序定点替换，避免重排整份 JSON。
function planJson(relPath, labels) {
  const text = read(relPath);
  const re = /^([ \t]*)"version":[ \t]*"([^"]*)"/gm;
  const changes = [];
  let count = 0;
  const next = text.replace(re, (line, indent, value) => {
    const label = labels[count];
    count += 1;
    if (!label) return line;
    changes.push({ label, from: value, to: target });
    return `${indent}"version": "${target}"`;
  });
  if (changes.length !== labels.length) {
    fail(`${relPath} 中应有 ${labels.length} 处顶层 "version" 字段，实际找到 ${changes.length} 处。`);
  }
  replacements.push({ path: relPath, next, changes });
}

planJson('src-tauri/tauri.conf.json', ['version']);
planJson('package.json', ['version']);
planJson('package-lock.json', ['version（根条目）', 'version（packages[""]）']);

// Cargo.toml：第一处 `version = "..."` 即 [package] 的版本。
{
  const relPath = 'src-tauri/Cargo.toml';
  const text = read(relPath);
  const re = /^version[ \t]*=[ \t]*"([^"]*)"/m;
  const m = re.exec(text);
  if (!m) fail(`${relPath} 中找不到 [package] 的 version 字段。`);
  replacements.push({
    path: relPath,
    next: text.replace(re, `version = "${target}"`),
    changes: [{ label: 'version', from: m[1], to: target }],
  });
}

// Cargo.lock：只改名为 freecontrol 的包条目。
{
  const relPath = 'src-tauri/Cargo.lock';
  const text = read(relPath);
  const re = /(\[\[package\]\]\r?\nname = "freecontrol"\r?\nversion = )"([^"]*)"/;
  const m = re.exec(text);
  if (!m) fail(`${relPath} 中找不到 freecontrol 包条目。`);
  replacements.push({
    path: relPath,
    next: text.replace(re, `$1"${target}"`),
    changes: [{ label: 'freecontrol', from: m[2], to: target }],
  });
}

// 原型：当前版本跟到目标；演示用最新版取下一个修订号，保持「可更新」演示。
{
  const relPath = 'ui-design/launcher-prototype.html';
  const text = read(relPath);
  const appRe = /(const APP_VERSION = ')([^']*)(';)/;
  const latestRe = /(const UPDATE_LATEST_VERSION = ')([^']*)(';)/;
  const app = appRe.exec(text);
  const latest = latestRe.exec(text);
  if (!app) fail(`${relPath} 中找不到 APP_VERSION 常量。`);
  if (!latest) fail(`${relPath} 中找不到 UPDATE_LATEST_VERSION 常量。`);
  const demoTarget = nextPatch(targetParts);
  replacements.push({
    path: relPath,
    next: text.replace(appRe, `$1${target}$3`).replace(latestRe, `$1${demoTarget}$3`),
    changes: [
      { label: 'APP_VERSION', from: app[2], to: target },
      { label: 'UPDATE_LATEST_VERSION', from: latest[2], to: demoTarget },
    ],
  });
}

// 先把所有文件的新内容算齐再写，避免中途失败留下半套取值。
for (const item of replacements) {
  writeFileSync(join(root, item.path), item.next, 'utf8');
}

console.log(`版本 ${current} → ${target}`);
for (const item of replacements) {
  for (const change of item.changes) {
    console.log(`  ${item.path}  ${change.label}: ${change.from} → ${change.to}`);
  }
}
console.log('更新日志需另行补写当前版本段；完成后运行 npm run check:version 复核。');
