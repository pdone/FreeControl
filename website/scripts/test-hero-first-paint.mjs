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

test('font fallback is gated and preserves the monospace prefix', () => {
  const stacks = [...tokens.matchAll(/--font-mono:\s*([^;]+);/g)].map((match) => match[1]);
  assert.equal(stacks.length, 2);
  assert(!stacks[0].includes('Alibaba'));
  assert.equal(stacks[1].replace("'Alibaba PuHuiTi 3', ", ''), stacks[0]);
  assert(tokens.slice(tokens.indexOf(':root.has-web-font')).includes(stacks[1]));
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
  const font = { ready: false };
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
    document: { documentElement: { classList: { contains: () => font.ready } } },
    webFontFacesCss: () => 'font-face',
    window: { setTimeout: (fn) => { const id = ++serial; timers.set(id, fn); return id; } },
  };
  vm.runInNewContext(code, context);
  return { api: context.api, frame, messages, timers, font, head, styles };
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

test('font injection waits for the site font class and runs once', () => {
  const s = setup();
  assert.equal(s.api.injectPreviewFont(), false);
  assert.equal(s.head.children.length, 0);
  s.font.ready = true;
  assert.equal(s.api.injectPreviewFont(), true);
  assert.equal(s.head.children.length, 1);
  assert.equal(s.head.children[0].textContent, 'font-face');
  assert.equal(s.api.injectPreviewFont(), true);
  assert.equal(s.head.children.length, 1);
});

console.log(`${checks} checks passed (${fileURLToPath(import.meta.url)})`);
