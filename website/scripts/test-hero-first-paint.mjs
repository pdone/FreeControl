import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';
import { transformSync } from 'esbuild';

const root = new URL('../../', import.meta.url);
const read = (file) => fs.readFileSync(new URL(file, root), 'utf8');
const prototype = read('ui-design/launcher-prototype.html');
const app = read('website/src/App.svelte');
const css = read('website/src/app.css');
const tokens = read('website/src/tokens.css');
const bootstrap = prototype.match(/<script id="protoThemeBootstrap">([\s\S]*?)<\/script>/)[1];
const origin = 'http://127.0.0.1:5178';
let checks = 0;
const test = (name, run) => {
  run();
  checks++;
  console.log(`PASS ${name}`);
};

for (const light of [false, true]) {
  for (const mode of ['light', 'dark', 'system', 'invalid', '', null]) {
    test(`bootstrap ${mode ?? 'missing'} / system ${light ? 'light' : 'dark'}`, () => {
      const dataset = {};
      vm.runInNewContext(bootstrap, {
        URLSearchParams,
        location: { search: mode === null ? '' : `?theme=${mode}` },
        document: { documentElement: { dataset } },
        window: { matchMedia: (query) => {
          assert.equal(query, '(prefers-color-scheme: light)');
          return { matches: light };
        } },
      });
      const expected = mode === 'system' ? (light ? 'light' : 'dark') : ['light', 'dark'].includes(mode) ? mode : undefined;
      assert.equal(dataset.theme, expected);
    });
  }
}

test('brand font is local-first and the self-hosted family uses another name', () => {
  const str = (pattern) => tokens.match(pattern)[1].replace(/\s+/g, ' ').trim();
  const sans = str(/--font-sans:\s*([^;]+);/);
  const local = str(/--font-brand-local:\s*([^;]+);/);
  const web = str(/--font-brand-web:\s*([^;]+);/);
  assert(sans.startsWith('var(--font-brand-local), var(--font-brand-web)'));
  assert(sans.endsWith('var(--font-sans-system)'));
  for (const name of ['"Alibaba PuHuiTi 3.0"', '"Alibaba PuHuiTi 3.0 55 Regular"', '"阿里巴巴普惠体 3.0"']) {
    assert(local.includes(name), `本机族名应包含 ${name}`);
  }
  assert.equal(web, "'Alibaba PuHuiTi 3'");
  assert(!tokens.includes('has-web-font'), '字体切换不再依赖 html 上的类');
});

test('monospace keeps its prefix and only lets the brand font fill missing CJK', () => {
  const prefix = 'ui-monospace, "SF Mono", Menlo, Consolas,';
  const site = tokens.match(/--font-mono:\s*([^;]+);/)[1].replace(/\s+/g, ' ');
  const proto = prototype.match(/--font-mono-default:\s*([^;]+);/)[1].replace(/\s+/g, ' ');
  for (const [label, stack] of [['站点', site], ['原型', proto]]) {
    assert(stack.startsWith(prefix), `${label}等宽栈前缀不应改变`);
    assert(stack.indexOf('var(--font-brand-local)') > stack.indexOf('Consolas'), `${label}普惠体必须排在等宽族之后`);
    assert(stack.endsWith('monospace'), `${label}等宽栈仍以 monospace 收尾`);
  }
  assert.equal((prototype.match(/ui-monospace/g) || []).length, 1, '原型不应有绕过 token 的等宽字面量');
});

test('prototype does not disable synthetic weight on its own', () => {
  // 只有一份 400 轮廓，600/700 全靠浏览器合成；原型若单独关掉合成，预览里的粗体会集体变细。
  // 先剥掉注释：这个开关的名字会出现在解释它为什么被移除的注释里。
  const strip = (source) => source.replace(/\/\*[\s\S]*?\*\//g, '');
  const synthesis = (source) => /font-synthesis\s*:\s*([^;]+);/.exec(strip(source))?.[1].trim() ?? null;
  assert.equal(synthesis(prototype), synthesis(css), '页面与原型必须同口径地对待合成加粗');
});

test('only one self-hosted weight is shipped', () => {
  const fontModule = read('website/src/lib/web-font.ts');
  assert.equal((fontModule.match(/puhuiti3-/g) || []).length, 1);
  assert(fontModule.includes('puhuiti3-55-subset.woff2'));
  assert.equal((fontModule.match(/@font-face\{/g) || []).length, 1);
  assert.equal((fontModule.match(/font-weight:/g) || []).length, 1);
  assert(!fs.existsSync(new URL('website/src/assets/fonts/puhuiti3-75-subset.woff2', root)));
  assert.deepEqual(fs.readdirSync(new URL('website/src/assets/fonts/', root)), ['puhuiti3-55-subset.woff2']);
});

test('hero preview stays at 1000x700 without a screenshot placeholder', () => {
  assert(!app.includes('hero-placeholder'));
  assert(!css.includes('hero-placeholder'));
  assert(/width: 1000px;\s+height: 700px;/.test(css));
  assert(tokens.includes('--preview-scale: 0.62;'));
  assert(tokens.includes('--preview-slot: calc(1000px * var(--preview-scale));'));
  assert(tokens.includes('--preview-h-slot: calc(700px * var(--preview-scale));'));
});

const handlers = [...prototype.matchAll(/window\.addEventListener\('message',\s*\(e\)\s*=>\s*\{([\s\S]*?)\n  \}\);/g)].map((match) => match[0]);
for (const [kind, value, ack] of [['lang', 'en', 'fc2:lang-applied'], ['theme', 'light', 'fc2:theme-applied']]) {
  const handler = handlers.find((text) => text.includes(`'fc2:${kind}'`));
  assert(handler);
  test(`${kind} validates source and value and acknowledges unchanged state`, () => {
    let listener;
    const sent = [];
    const applied = [];
    const parent = { postMessage: (message, target) => sent.push({ message, target }) };
    const context = {
      window: { parent, addEventListener: (_, fn) => { listener = fn; } },
      location: { origin },
      setPref: (v) => applied.push(v),
      themeMode: 'dark',
      syncThemeSeg: () => {},
      applyTheme: (animate) => { assert.equal(animate, false); applied.push('theme'); },
    };
    vm.runInNewContext(handler, context);
    const data = { type: `fc2:${kind}`, [kind === 'lang' ? 'lang' : 'mode']: value, requestId: 7 };
    listener({ source: {}, origin, data });
    listener({ source: parent, origin: 'https://invalid.example', data });
    listener({ source: parent, origin, data: { ...data, [kind === 'lang' ? 'lang' : 'mode']: 'invalid' } });
    assert.equal(sent.length, 0);
    assert.equal(applied.length, 0);
    listener({ source: parent, origin, data });
    listener({ source: parent, origin, data });
    assert.equal(sent.length, 2);
    for (const item of sent) {
      assert.equal(item.target, origin);
      assert.equal(item.message.type, ack);
      assert.equal(item.message.requestId, 7);
    }
    for (const requestId of [undefined, -1, 0, 1.5, '7']) listener({ source: parent, origin, data: { ...data, requestId } });
    assert.equal(sent.length, 2);
  });
}

const startAck = app.indexOf('  let themeAck');
const endAck = app.indexOf('  function onFrameLoad', startAck);
const startFont = app.indexOf('  function injectPreviewFont');
const endFont = app.indexOf('  function onFrameLoad', startFont);
assert(startAck > 0 && endAck > startAck && startFont > 0 && endFont > startFont);
const code = transformSync(app.slice(startAck, endAck) + app.slice(startFont, endFont) + `
globalThis.api = { onPreviewMessage, pushPreviewTheme, injectPreviewFont, ackState: () => themeAck?.id ?? null };`, { loader: 'ts' }).code;

function setup() {
  let serial = 0;
  const timers = new Map();
  const messages = [];
  const head = { children: [], appendChild(node) { this.children.push(node); } };
  const styles = new Map();
  const contentDocument = {
    head,
    getElementById: (id) => styles.get(id) ?? null,
    createElement: () => {
      const node = { textContent: '' };
      Object.defineProperty(node, 'id', { set(value) { node._id = value; styles.set(value, node); }, get() { return node._id; } });
      return node;
    },
  };
  const frame = { contentWindow: { postMessage: (data, target) => messages.push({ data, target }) }, contentDocument };
  const context = {
    frameEl: frame,
    previewArmed: true,
    themeMode: { value: 'light' },
    get: (store) => store.value,
    location: { origin },
    webFontFacesCss: () => 'font-face',
    window: { setTimeout: (fn) => { const id = ++serial; timers.set(id, fn); return id; } },
  };
  vm.runInNewContext(code, context);
  return { api: context.api, frame, messages, timers, head, styles };
}

test('theme acknowledgement must match the current request', () => {
  const s = setup();
  void s.api.pushPreviewTheme('light');
  const id = s.messages.at(-1).data.requestId;
  assert.equal(id, 1);
  s.api.onPreviewMessage({ origin: 'https://invalid.example', source: s.frame.contentWindow, data: { type: 'fc2:theme-applied', requestId: id } });
  s.api.onPreviewMessage({ origin, source: {}, data: { type: 'fc2:theme-applied', requestId: id } });
  s.api.onPreviewMessage({ origin, source: s.frame.contentWindow, data: { type: 'fc2:theme-applied', requestId: id + 1 } });
  assert.equal(s.api.ackState(), id);
  s.api.onPreviewMessage({ origin, source: s.frame.contentWindow, data: { type: 'fc2:theme-applied', requestId: id } });
  assert.equal(s.api.ackState(), null);
});

test('transition timeout releases only its own wait', () => {
  const s = setup();
  void s.api.pushPreviewTheme('light');
  const first = s.messages.at(-1).data.requestId;
  const firstTimer = [...s.timers.values()][0];
  void s.api.pushPreviewTheme('light');
  const second = s.messages.at(-1).data.requestId;
  assert.notEqual(first, second);
  assert.equal(s.api.ackState(), second);
  firstTimer();
  assert.equal(s.api.ackState(), second);
  s.api.onPreviewMessage({ origin, source: s.frame.contentWindow, data: { type: 'fc2:theme-applied', requestId: second } });
  assert.equal(s.api.ackState(), null);
});

test('font injection needs no ready state and runs once', () => {
  const s = setup();
  assert.equal(s.api.injectPreviewFont(), true);
  assert.equal(s.head.children.length, 1);
  assert.equal(s.head.children[0].textContent, 'font-face');
  assert.equal(s.api.injectPreviewFont(), true);
  assert.equal(s.head.children.length, 1);
});

console.log(`${checks} checks passed (${fileURLToPath(import.meta.url)})`);
