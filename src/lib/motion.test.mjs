import test from 'node:test';
import assert from 'node:assert/strict';
import { cubicOut } from 'svelte/easing';
import { DIALOG_FADE, DIALOG_SCALE, dialogScale } from './motion.ts';

test('DIALOG_SCALE 与 DIALOG_FADE 共用时长，start 0.94', () => {
  assert.equal(DIALOG_SCALE.start, 0.94);
  assert.equal(DIALOG_SCALE.duration, DIALOG_FADE.duration);
  // node 环境无 window → 非减少动态效果路径
  assert.equal(DIALOG_SCALE.duration, 160);
});

test('dialogScale 端点：t=0 scale(0.94)，t=1 scale(1)，中点线性插值', () => {
  const { css } = dialogScale(null, { duration: 160, start: 0.94 });
  assert.equal(css(0, 1), 'transform: scale(0.94)');
  assert.equal(css(1, 0), 'transform: scale(1)');
  assert.equal(css(0.5, 0.5), 'transform: scale(0.97)');
});

test('dialogScale css 只含 transform、不含 opacity（透明度单一来源是遮罩淡入）', () => {
  const { css } = dialogScale(null, { duration: 160, start: 0.94 });
  for (const t of [0, 0.25, 0.5, 0.75, 1]) {
    const out = css(t, 1 - t);
    assert.ok(out.startsWith('transform: scale('), out);
    assert.ok(!out.includes('opacity'), out);
  }
});

test('dialogScale 缓动与 fade 默认一致（cubicOut），时长可覆写（减少动态效果 1ms 路径）', () => {
  const cfg = dialogScale(null, { duration: 1, start: 0.94 });
  assert.equal(cfg.duration, 1);
  assert.equal(cfg.easing, cubicOut);
  assert.equal(cfg.css(1, 0), 'transform: scale(1)');
});

test('dialogScale 缺省参数取 DIALOG_SCALE', () => {
  const cfg = dialogScale(null);
  assert.equal(cfg.duration, DIALOG_SCALE.duration);
  assert.equal(cfg.css(0, 1), `transform: scale(${DIALOG_SCALE.start})`);
});