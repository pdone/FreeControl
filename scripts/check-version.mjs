// 版本与发行文档校验：构建期版本、各处副本、原型演示版本与两份更新日志。
//
// 用法：
//   node scripts/check-version.mjs
//   npm run check:version
//
// 基准是 src-tauri/tauri.conf.json（发行 tag、产物名与 getVersion() 的唯一来源）。
// 全部一致时保持安静并以 0 退出；有任一不一致时一次性列出全部问题并以 1 退出。
//
// 依赖：仅 Node 内置模块。

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const VERSION_RE = /^(\d+)\.(\d+)\.(\d+)$/;
const errors = [];

function read(relPath) {
  try {
    return readFileSync(join(root, relPath), 'utf8');
  } catch (error) {
    errors.push(`${relPath} 读取失败：${error.message}`);
    return null;
  }
}

function parseVersion(value) {
  const m = VERSION_RE.exec(value ?? '');
  return m ? m.slice(1).map(Number) : null;
}

function compareVersion(a, b) {
  for (let i = 0; i < 3; i += 1) {
    if (a[i] !== b[i]) return a[i] - b[i];
  }
  return 0;
}

function jsonVersions(text) {
  return [...text.matchAll(/^[ \t]*"version":[ \t]*"([^"]*)"/gm)].map((m) => m[1]);
}

function cargoVersion(text) {
  const m = /^version[ \t]*=[ \t]*"([^"]*)"/m.exec(text);
  return m ? m[1] : null;
}

function lockVersion(text) {
  const m = /\[\[package\]\]\r?\nname = "freecontrol"\r?\nversion = "([^"]*)"/.exec(text);
  return m ? m[1] : null;
}

function protoConst(text, name) {
  const m = new RegExp(`const ${name} = '([^']*)'`).exec(text);
  return m ? m[1] : null;
}

const tauriText = read('src-tauri/tauri.conf.json');
const current = tauriText ? jsonVersions(tauriText)[0] ?? null : null;
const currentParts = parseVersion(current);
if (!current) {
  errors.push('src-tauri/tauri.conf.json 中找不到顶层 "version" 字段。');
} else if (!currentParts) {
  errors.push(`src-tauri/tauri.conf.json 的版本不是三段式数字：${current}`);
}

function expectField(relPath, label, actual) {
  if (actual === null || actual === undefined) {
    errors.push(`${relPath} 中找不到 ${label}。`);
    return;
  }
  if (current && actual !== current) {
    errors.push(`${relPath} 的 ${label} 为 ${actual}，应为 ${current}。`);
  }
}

if (currentParts) {
  const cargoToml = read('src-tauri/Cargo.toml');
  if (cargoToml !== null) expectField('src-tauri/Cargo.toml', '[package] 的 version', cargoVersion(cargoToml));

  const cargoLock = read('src-tauri/Cargo.lock');
  if (cargoLock !== null) expectField('src-tauri/Cargo.lock', 'freecontrol 的 version', lockVersion(cargoLock));

  const pkg = read('package.json');
  if (pkg !== null) expectField('package.json', 'version', jsonVersions(pkg)[0] ?? null);

  const lock = read('package-lock.json');
  if (lock !== null) {
    const versions = jsonVersions(lock);
    expectField('package-lock.json', '根条目的 version', versions[0] ?? null);
    expectField('package-lock.json', 'packages[""] 的 version', versions[1] ?? null);
  }

  const proto = read('ui-design/launcher-prototype.html');
  if (proto !== null) {
    expectField('ui-design/launcher-prototype.html', 'APP_VERSION', protoConst(proto, 'APP_VERSION'));

    const demo = protoConst(proto, 'UPDATE_LATEST_VERSION');
    const demoParts = parseVersion(demo);
    if (!demo) {
      errors.push('ui-design/launcher-prototype.html 中找不到 UPDATE_LATEST_VERSION 常量。');
    } else if (!demoParts) {
      errors.push(`ui-design/launcher-prototype.html 的 UPDATE_LATEST_VERSION 不是三段式数字：${demo}`);
    } else if (compareVersion(demoParts, currentParts) <= 0) {
      errors.push(`ui-design/launcher-prototype.html 的 UPDATE_LATEST_VERSION 为 ${demo}，应高于当前版本 ${current}。`);
    }
  }

  const docs = ['Update.md', 'Update.en.md'];
  const missingDocs = [];
  for (const doc of docs) {
    const text = read(doc);
    if (text === null) continue;
    const heading = `## v${current}`;
    const found = text.split(/\r?\n/).some((line) => line.trim() === heading);
    if (!found) {
      missingDocs.push(doc);
      errors.push(`${doc} 缺少当前版本的版本标题 "${heading}"。`);
    }
  }
  if (missingDocs.length > 0) {
    errors.push(`提示：先在 ${missingDocs.join(' 与 ')} 补写 "## v${current}" 一节，再重跑本检查。`);
  }
}

if (errors.length > 0) {
  console.error('版本与发行文档校验未通过：');
  for (const error of errors) console.error(`- ${error}`);
  process.exit(1);
}
