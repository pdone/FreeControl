// 原型中英文案一致性检查（config ㉙ 的语言层）：
// - 原型里每条带中文的源串（标记文本、承载文案的属性、脚本字符串）都必须在映射里有条目；
// - 映射里不得有死条目（源串已经不在原型里）；
// - 与产品 zh.ts 逐字相同的条目，英文须与 en.ts 同键取值一致（防术语漂移）；
// - `X的说明` 形式的可访问名须由 `X` 的译文派生（`About <X>`），与产品 *.helpLabel 同句式。
//
// 依赖：Node 内置模块 + Node 的类型剥离（`--experimental-strip-types`），不引入依赖。
// 用法：npm run check:i18n:proto

import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { zh } from '../src/lib/i18n/zh.ts';
import { en } from '../src/lib/i18n/en.ts';

const here = dirname(fileURLToPath(import.meta.url));
export const PROTO_PATH = resolve(here, '..', 'ui-design', 'launcher-prototype.html');

const CJK = /[\u3400-\u9fff]/;
export const ATTRS = ['aria-label', 'placeholder', 'data-tip', 'title', 'alt'];
// 自名语言选项：英文界面里也照原样写，故不进映射
const WHITELIST = new Set(['简体中文']);

const DICT_BLOCK = /<script[^>]*id="protoI18n"[^>]*>([\s\S]*?)<\/script>/i;

/** 读出原型里的映射表（JSON 块）。 */
export function readDict(html) {
  const m = html.match(DICT_BLOCK);
  if (!m) throw new Error('原型里找不到 id="protoI18n" 的映射块');
  return JSON.parse(m[1]);
}

// 把 `${...}` 归一成 `{0}` `{1}`：占位符让两种语言的词序各自成立，也让映射条目可被精确匹配。
function normalizeTemplate(raw) {
  let out = '';
  let i = 0;
  let idx = 0;
  while (i < raw.length) {
    if (raw[i] === '$' && raw[i + 1] === '{') {
      let depth = 1;
      let j = i + 2;
      const inner = [];
      while (j < raw.length && depth > 0) {
        const c = raw[j];
        if (c === '{') depth++;
        else if (c === '}') depth--;
        if (depth > 0) inner.push(c);
        j++;
      }
      out += '{' + idx++ + '}';
      i = j;
      continue;
    }
    out += raw[i];
    i++;
  }
  return out;
}

// 扫出一段脚本里的全部字符串字面量（跳过注释）；模板字面量里的 `${...}` 递归再扫，
// 因为像 `${oc ? \`<span>已启用设备覆盖</span>\` : ''}` 这样的嵌套文案同样会渲染出来。
function scanScript(src, sink, depth = 0) {
  if (depth > 6) return;
  let i = 0;
  const n = src.length;
  while (i < n) {
    const c = src[i];
    if (c === '/' && src[i + 1] === '/') {
      const nl = src.indexOf('\n', i);
      i = nl < 0 ? n : nl;
      continue;
    }
    if (c === '/' && src[i + 1] === '*') {
      const end = src.indexOf('*/', i + 2);
      i = end < 0 ? n : end + 2;
      continue;
    }
    if (c === '"' || c === "'" || c === '`') {
      const quote = c;
      let j = i + 1;
      let raw = '';
      while (j < n) {
        const d = src[j];
        if (d === '\\') {
          raw += src[j] + (src[j + 1] ?? '');
          j += 2;
          continue;
        }
        if (d === quote) break;
        if (quote === '`' && d === '$' && src[j + 1] === '{') {
          let dep = 1;
          let k = j + 2;
          const inner = [];
          while (k < n && dep > 0) {
            const e = src[k];
            if (e === '{') dep++;
            else if (e === '}') dep--;
            if (dep > 0) inner.push(e);
            else break;
            k++;
          }
          scanScript(inner.join(''), sink, depth + 1);
          raw += '${' + inner.join('') + '}';
          j = dep > 0 ? n : k + 1;
          continue;
        }
        raw += d;
        j++;
      }
      sink(raw, quote === '`');
      i = j + 1;
      continue;
    }
    i++;
  }
}

function splitHtml(s) {
  return s
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/<style[\s\S]*?<\/style>/gi, '')
    .replace(/<script[\s\S]*?<\/script>/gi, '');
}

function textNodes(markup) {
  const out = [];
  // 末尾那段正文后面没有标签了（如 `<i …></i>已就绪 …`），所以还要认 `$` 收尾
  for (const m of markup.matchAll(/(?:>|^)([^<>]+)(?=<|$)/g)) out.push(m[1]);
  return out;
}

function attrValues(markup) {
  const out = [];
  for (const name of ATTRS) {
    const re = new RegExp(name + '="([^"]*)"', 'g');
    for (const m of markup.matchAll(re)) out.push(m[1]);
  }
  return out;
}

/** 收集原型里全部带中文的源串（已归一化、去重）。 */
export function collectSources(html) {
  const sources = new Set();
  const add = (raw) => {
    if (!raw) return;
    const s = raw.trim();
    if (!s || !CJK.test(s)) return;
    if (/^\{\d+\}$/.test(s)) return; // 纯占位符（如 data-tip="${tip}"）没有正文可译
    if (WHITELIST.has(s)) return;
    sources.add(s);
  };

  const noDict = html.replace(DICT_BLOCK, '');
  const markup = splitHtml(noDict);
  for (const t of textNodes(markup)) add(t);
  for (const v of attrValues(markup)) add(v);

  const scripts = [...noDict.matchAll(/<script[^>]*>([\s\S]*?)<\/script>/gi)].map((m) => m[1]);
  for (const src of scripts) {
    scanScript(src, (raw, isTemplate) => {
      const tpl = isTemplate ? normalizeTemplate(raw) : raw;
      // 字面量里含标签说明它是 innerHTML 片段：按标记再拆一层，取出正文与属性值
      if (tpl.includes('<') && tpl.includes('>')) {
        const frag = splitHtml(tpl);
        for (const t of textNodes(frag)) add(t);
        for (const v of attrValues(frag)) add(v);
        return;
      }
      add(tpl);
    });
  }
  return sources;
}

function report(issues) {
  console.error('[check-proto-i18n] 失败');
  for (const issue of issues) console.error(`- ${issue}`);
  process.exit(1);
}

function main() {
  const html = readFileSync(PROTO_PATH, 'utf8');
  const dict = readDict(html);
  const sources = collectSources(html);
  const zhValues = new Set(Object.values(zh));

  const missing = [...sources].filter((s) => !Object.prototype.hasOwnProperty.call(dict, s));
  const dead = Object.keys(dict).filter((k) => !sources.has(k));

  // 与产品字典逐字相同的条目：英文必须取产品同键的译文，否则同一句话在原型与产品里叫法不同
  const drift = [];
  for (const [src, out] of Object.entries(dict)) {
    if (!zhValues.has(src)) continue;
    const key = Object.keys(zh).find((k) => zh[k] === src);
    if (key && en[key] !== undefined && en[key] !== out) drift.push(`${src} ⇒ ${out}（产品：${en[key]}）`);
  }

  // `X的说明` 由 `X` 的译文派生，避免这些可访问名各写一遍还写歪；
  // 产品字典里已经给出整句的（如 `界面语言的说明`）以产品为准，由上面那条一致性校验管
  const derive = [];
  for (const [src, out] of Object.entries(dict)) {
    const m = src.match(/^(.+)的说明$/);
    if (!m || zhValues.has(src)) continue;
    const base = dict[m[1]];
    if (base === undefined) continue;
    const want = 'About ' + base;
    if (out !== want) derive.push(`${src} ⇒ ${out}（应为 ${want}）`);
  }

  const blank = Object.entries(dict).filter(([, v]) => typeof v !== 'string' || v.trim() === '');
  const issues = [];
  if (missing.length) issues.push(`未覆盖的源串 ${missing.length} 条：${missing.join(' | ')}`);
  if (dead.length) issues.push(`死条目 ${dead.length} 条：${dead.join(' | ')}`);
  if (drift.length) issues.push(`与产品译文不一致 ${drift.length} 条：${drift.join('；')}`);
  if (derive.length) issues.push(`「的说明」派生不一致 ${derive.length} 条：${derive.join('；')}`);
  if (blank.length) issues.push(`空译文 ${blank.length} 条：${blank.map(([k]) => k).join(' | ')}`);
  if (issues.length) report(issues);

  console.log(
    `[check-proto-i18n] 通过：${sources.size} 条源串全有译文，${Object.keys(dict).length} 条条目无死键，` +
      `与产品字典逐字相同的条目译文一致，可访问名派生一致`,
  );
}

const invoked = process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url));
if (invoked) main();
