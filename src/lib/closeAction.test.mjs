import test from 'node:test';
import assert from 'node:assert/strict';
import { decideCloseAction } from './closeAction.ts';

test('偏好开启且托盘可用时收进托盘', () => {
  assert.equal(
    decideCloseAction({ closeToTray: true, trayReady: true, quitting: false }),
    'hide',
  );
});

test('偏好关闭时直接退出', () => {
  assert.equal(
    decideCloseAction({ closeToTray: false, trayReady: true, quitting: false }),
    'quit',
  );
});

test('托盘不可用时即使偏好开启也直接退出', () => {
  assert.equal(
    decideCloseAction({ closeToTray: true, trayReady: false, quitting: false }),
    'quit',
  );
});

test('退出中永远退出，不会被收进托盘拦截', () => {
  assert.equal(
    decideCloseAction({ closeToTray: true, trayReady: true, quitting: true }),
    'quit',
  );
});
