#!/usr/bin/env node
/**
 * 生成原型内联的 lucide 图标子集。
 *
 * 输入（每次运行现扫，不手写清单）：
 *   - ui-design/launcher-prototype.html 的 `data-lucide="kebab-name"` / `icon:'kebab-name'` / `primaryIcon:'kebab-name'`
 *   - 脚本内补录清单（模板拼接出来的取值，扫不到）
 *
 * 输出（整段重写）：
 *   - ui-design/launcher-prototype.html 里的 `<script id="protoIcons">` 块（kebab 键 + createIcons 垫片）
 *
 * 图形数据取自仓库根 `node_modules/lucide-svelte/dist/icons/<kebab>.svelte` 的 `iconNode` 字面量。
 * 版本单一来源：产品 package.json 的 `lucide-svelte` 取值（实装版本写进生成物头部）。
 * 官网仓另有一套只扫描其 src 的生成器，两边各自维护，不共享扫描集合。
 *
 * 用法：
 *   node scripts/gen-icon-subset.mjs           重新生成原型内联子集
 *   node scripts/gen-icon-subset.mjs --check   只比对，不一致则非零退出（不改盘）
 */
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const PROTOTYPE_PATH = join(ROOT, 'ui-design/launcher-prototype.html');
const ICONS_DIR = join(ROOT, 'node_modules/lucide-svelte/dist/icons');

const CHECK = process.argv.includes('--check');

/** 模板拼接出来的图标名（扫不到字面量），与扫描结果取并集。 */
const SUPPLEMENT = [
  'usb', // ${d.type==='usb'?'usb':'wifi'}
  'wifi',
  'minimize-2', // ${paramsExpanded ? 'minimize-2' : 'maximize-2'}
  'maximize-2',
  'rotate-cw', // cfg.primaryIcon
  'circle-alert', // cfg.icon
  'zap', // 连接按钮：connected ? 'zap-off' : 'zap'
  'zap-off',
];

const KEBAB = /^[a-z0-9][a-z0-9-]*$/;

function fail(message) {
  console.error(`✗ ${message}`);
  process.exit(1);
}

function read(path) {
  return readFileSync(path, 'utf8');
}

/** 沿用目标文件的换行符（Windows 检出是 CRLF），免得生成物在 diff 里整段变色。 */
function eolOf(text) {
  return text.includes('\r\n') ? '\r\n' : '\n';
}

function stripEol(text) {
  return text.replace(/\r\n/g, '\n');
}

function collectMatches(text, re, onName) {
  for (const m of text.matchAll(re)) onName(m[1]);
}

/** 扫描出所有被引用的图标名，统一成 kebab。 */
function collectIconNames() {
  const kebab = new Set();
  const pushKebab = (name) => {
    if (KEBAB.test(name)) kebab.add(name);
  };

  // 原型：标记 + 脚本里写死的取值
  if (!existsSync(PROTOTYPE_PATH)) fail(`找不到 ${relative(ROOT, PROTOTYPE_PATH)}`);
  const proto = read(PROTOTYPE_PATH);
  collectMatches(proto, /data-lucide="([^"]*)"/g, pushKebab);
  collectMatches(proto, /\bicon:\s*'([^']*)'/g, pushKebab);
  collectMatches(proto, /\bprimaryIcon:\s*'([^']*)'/g, pushKebab);

  for (const name of SUPPLEMENT) kebab.add(name);

  return [...kebab].sort();
}

function lucideVersion() {
  const rootPkg = JSON.parse(read(join(ROOT, 'package.json')));
  const spec = rootPkg.dependencies?.['lucide-svelte'] ?? rootPkg.devDependencies?.['lucide-svelte'];
  if (!spec) fail('根 package.json 里没有 lucide-svelte 依赖 —— 子集的版本取值无来源');

  const pkgPath = join(ROOT, 'node_modules/lucide-svelte/package.json');
  if (!existsSync(pkgPath)) {
    fail(
      '找不到 node_modules/lucide-svelte —— 重新生成子集需要仓库根的 lucide-svelte（生成物已入库，站点构建不受影响）',
    );
  }
  const installed = JSON.parse(read(pkgPath)).version;
  return { spec, installed };
}

/** 解析 lucide-svelte 单个图标文件里的 `iconNode` 字面量。 */
function readIconNode(name) {
  const path = join(ICONS_DIR, `${name}.svelte`);
  if (!existsSync(path)) return { missing: true };

  const source = read(path);
  const m = source.match(/const iconNode = (\[[\s\S]*?\]);/);
  if (!m) fail(`lucide-svelte 的 ${name}.svelte 里找不到 iconNode 字面量`);

  let children;
  try {
    children = JSON.parse(m[1]);
  } catch {
    fail(`lucide-svelte 的 ${name}.svelte 里 iconNode 不是合法 JSON，需要更新本脚本的解析方式`);
  }
  return { node: ['svg', {}, children] };
}

const PROTO_BLOCK_RE =
  /(?:<!-- 由 scripts\/gen-icon-subset\.mjs 生成[\s\S]*?-->\r?\n)?<script id="protoIcons">[\s\S]*?<\/script>/;
const PROTO_CDN_RE =
  /[ \t]*<script src="https:\/\/unpkg\.com\/lucide@[^"]*\/dist\/umd\/lucide\.js"><\/script>/;

function buildProtoBlock(names, nodes, version, eol) {
  const lines = names.map((name) => `    "${name}": ${JSON.stringify(nodes[name])},`);
  return [
    '<!-- 由 scripts/gen-icon-subset.mjs 生成，勿手改。数据源：lucide-svelte@' + version + '（ISC）',
    '     图标子集内联于此，原型因此自足：断网、双击即可打开，不请求任何外部资源。 -->',
    '<script id="protoIcons">',
    '(function () {',
    '  var ICONS = {',
    ...lines,
    '  };',
    "  var SVG_NS = 'http://www.w3.org/2000/svg';",
    '',
    '  function toNode(entry) {',
    "    var el = document.createElementNS(SVG_NS, entry[0]);",
    '    var attrs = entry[1] || {};',
    '    for (var k in attrs) {',
    '      if (Object.prototype.hasOwnProperty.call(attrs, k)) el.setAttribute(k, String(attrs[k]));',
    '    }',
    '    var children = entry[2] || [];',
    '    for (var i = 0; i < children.length; i++) el.appendChild(toNode(children[i]));',
    '    return el;',
    '  }',
    '',
    '  function toSvg(node, name) {',
    "    var svg = document.createElementNS(SVG_NS, 'svg');",
    "    svg.setAttribute('xmlns', SVG_NS);",
    "    svg.setAttribute('class', 'lucide lucide-' + name);",
    "    svg.setAttribute('width', '24');",
    "    svg.setAttribute('height', '24');",
    "    svg.setAttribute('viewBox', '0 0 24 24');",
    "    svg.setAttribute('fill', 'none');",
    "    svg.setAttribute('stroke', 'currentColor');",
    "    svg.setAttribute('stroke-width', '2');",
    "    svg.setAttribute('stroke-linecap', 'round');",
    "    svg.setAttribute('stroke-linejoin', 'round');",
    '    var children = node[2] || [];',
    '    for (var i = 0; i < children.length; i++) svg.appendChild(toNode(children[i]));',
    '    return svg;',
    '  }',
    '',
    '  // 只实现原型用到的那一个方法（`lucide.createIcons()` 无参调用）。',
    '  window.lucide = {',
    '    createIcons: function () {',
    "      var hosts = document.querySelectorAll('[data-lucide]');",
    '      for (var i = 0; i < hosts.length; i++) {',
    '        var el = hosts[i];',
    "        var name = el.getAttribute('data-lucide');",
    '        var node = ICONS[name];',
    '        if (!node) {',
    // 控制台告警不走界面文案层（不参与中英切换），故用 ASCII 措辞，免得被 i18n 检查当成 UI 源串
    "          console.warn('[icons] unknown icon: ' + name);",
    '          continue;',
    '        }',
    '        var svg = toSvg(node, name);',
    '        for (var j = 0; j < el.attributes.length; j++) {',
    '          var attr = el.attributes[j];',
    "          if (attr.name === 'data-lucide') continue;",
    '          svg.setAttribute(attr.name, attr.value);',
    '        }',
    '        el.parentNode.replaceChild(svg, el);',
    '      }',
    '    },',
    '  };',
    '})();',
    '</script>',
  ].join(eol);
}

function main() {
  const names = collectIconNames();

  const missing = [];
  const nodes = {};
  for (const name of names) {
    const { missing: notFound, node } = readIconNode(name);
    if (notFound) missing.push(name);
    else nodes[name] = node;
  }
  if (missing.length) {
    fail(
      `lucide-svelte 里找不到这些图标：${missing.join('、')}\n` +
        '  改标记里的名字（如 `alert-triangle` → `triangle-alert`），或从扫描来源里撤掉它。',
    );
  }

  const { spec, installed } = lucideVersion();

  const proto = read(PROTOTYPE_PATH);
  const hasBlock = PROTO_BLOCK_RE.test(proto);
  const hasCdn = PROTO_CDN_RE.test(proto);

  if (!hasBlock && !hasCdn) {
    fail(
      `原型里既没有 protoIcons 块、也没有可替换的 lucide CDN 脚本：${relative(ROOT, PROTOTYPE_PATH)}`,
    );
  }

  const protoEol = eolOf(proto);
  const protoBlock = buildProtoBlock(names, nodes, installed, protoEol);

  const nextProto = hasBlock
    ? proto.replace(PROTO_BLOCK_RE, () => protoBlock)
    : proto.replace(PROTO_CDN_RE, () => protoBlock);

  if (CHECK) {
    const problems = [];
    if (!hasBlock) problems.push(`${relative(ROOT, PROTOTYPE_PATH)} 里没有 protoIcons 块`);
    else if (stripEol(nextProto) !== stripEol(proto))
      problems.push(`${relative(ROOT, PROTOTYPE_PATH)} 的 protoIcons 块与扫描结果不一致`);

    if (problems.length) {
      console.error('✗ 图标子集与源码不一致，先跑 `npm run icons:ui`：');
      for (const p of problems) console.error(`  - ${p}`);
      process.exit(1);
    }
    console.log(`✓ 图标子集一致：${names.length} 个图标（lucide-svelte@${installed}）`);
    return;
  }

  if (nextProto !== proto) writeFileSync(PROTOTYPE_PATH, nextProto, 'utf8');

  console.log(
    `✓ 图标子集已生成：${names.length} 个图标（lucide-svelte@${installed}，package.json 写 ${spec}）`,
  );
  if (nextProto !== proto) console.log('  - ui-design/launcher-prototype.html');
}

main();
