import test from 'node:test';
import assert from 'node:assert/strict';
import { flippedDirection, moveToIndex, sameOrder, sortDevices } from './deviceSort.ts';
import { acceptStatusSequence, reduceDeviceList } from './deviceSnapshot.ts';

const device = (id, alias, extra = {}) => ({
  id,
  alias,
  kind: 'usb',
  serial: id,
  ip: '',
  port: 0,
  ...extra,
});

const prefs = (
  sortMode,
  manualOrder,
  directions = { status: 'asc', name: 'asc', lastConnected: 'desc' },
) => ({ sortMode, directions, manualOrder, revision: 1 });

test('manual mode follows the saved manual order and leaves the input untouched', () => {
  const devices = [device('a', 'A'), device('b', 'B'), device('c', 'C')];
  const snapshot = [...devices];
  const result = sortDevices(devices, {}, prefs('manual', ['c', 'a', 'b']), 'zh');
  assert.deepEqual(
    result.map((d) => d.id),
    ['c', 'a', 'b'],
  );
  assert.deepEqual(devices, snapshot, '输入数组不得被原地修改');
});

test('status mode ranks connected, idle, unknown, offline and keeps manual order within a group', () => {
  const devices = [device('a', 'A'), device('b', 'B'), device('c', 'C'), device('d', 'D')];
  const statuses = { a: 'offline', b: 'connected', c: 'idle' };
  const result = sortDevices(devices, statuses, prefs('status', ['d', 'c', 'b', 'a']), 'zh');
  assert.deepEqual(
    result.map((d) => d.id),
    ['b', 'c', 'd', 'a'],
  );
});

test('status mode keeps the last known status when the probe is stale', () => {
  const devices = [device('a', 'A'), device('b', 'B')];
  const result = sortDevices(devices, { a: 'idle', b: 'offline' }, prefs('status', ['a', 'b']), 'zh');
  assert.deepEqual(result.map((d) => d.id), ['a', 'b']);
});

test('name mode compares digits by value and falls back to manual order for equal keys', () => {
  const devices = [
    device('a', '设备 10'),
    device('b', '设备 2'),
    device('c', '设备 2'),
    device('d', 'Device 1'),
  ];
  const ids = sortDevices(devices, {}, prefs('name', ['c', 'a', 'b', 'd']), 'zh').map((d) => d.id);
  // 数字按数值比较：「设备 2」在「设备 10」之前。
  assert.ok(ids.indexOf('a') > ids.indexOf('b'));
  assert.ok(ids.indexOf('a') > ids.indexOf('c'));
  // 同名（含「设备 2」两次）按手动顺序：c 在 b 之前。
  assert.ok(ids.indexOf('c') < ids.indexOf('b'));
});

test('name mode uses the requested locale for Chinese aliases', () => {
  const devices = [device('a', '阿机'), device('b', '波机'), device('c', '安机')];
  const zh = sortDevices(devices, {}, prefs('name', ['a', 'b', 'c']), 'zh');
  const collator = new Intl.Collator('zh', { numeric: true, sensitivity: 'accent' });
  const expected = [...devices].sort((x, y) => collator.compare(x.alias, y.alias)).map((d) => d.id);
  assert.deepEqual(zh.map((d) => d.id), expected);
});

test('lastConnected mode is newest-first with missing values last, ties by manual order', () => {
  const devices = [
    device('a', 'A', { lastSessionStartedAt: 100 }),
    device('b', 'B', { lastSessionStartedAt: null }),
    device('c', 'C', { lastSessionStartedAt: 300 }),
    device('d', 'D', { lastSessionStartedAt: 100 }),
    device('e', 'E'),
  ];
  const result = sortDevices(devices, {}, prefs('lastConnected', ['b', 'd', 'a', 'e', 'c']), 'zh');
  assert.deepEqual(
    result.map((d) => d.id),
    ['c', 'd', 'a', 'b', 'e'],
  );
});

test('direction flips each non-manual mode without reversing manual ties', () => {
  const devices = [
    device('a', 'A', { lastSessionStartedAt: 100 }),
    device('b', 'B', { lastSessionStartedAt: 300 }),
    device('c', 'C', { lastSessionStartedAt: 100 }),
  ];
  const ids = ['c', 'b', 'a'];

  // 状态降序：离线 → 检测中 → 在线 → 已连接。
  const statuses = { a: 'offline', b: 'connected', c: 'idle' };
  assert.deepEqual(
    sortDevices(devices, statuses, prefs('status', ids, { status: 'desc', name: 'asc', lastConnected: 'desc' }), 'en').map((d) => d.id),
    ['a', 'c', 'b'],
  );

  // 名称降序：Z → A；同名仍按手动顺序。
  assert.deepEqual(
    sortDevices(devices, {}, prefs('name', ids, { status: 'asc', name: 'desc', lastConnected: 'desc' }), 'en').map((d) => d.id),
    ['c', 'b', 'a'],
  );

  // 最后连接升序：最旧优先；同时间按手动顺序；无记录仍在最后。
  const withMissing = [...devices, device('d', 'D')];
  assert.deepEqual(
    sortDevices(
      withMissing,
      {},
      prefs('lastConnected', ['d', 'c', 'b', 'a'], { status: 'asc', name: 'asc', lastConnected: 'asc' }),
      'en',
    ).map((d) => d.id),
    ['c', 'a', 'b', 'd'],
  );
});

test('flippedDirection toggles asc and desc', () => {
  assert.equal(flippedDirection('asc'), 'desc');
  assert.equal(flippedDirection('desc'), 'asc');
});

test('moveToIndex uses the index in the sequence without the dragged id', () => {
  const ids = ['a', 'b', 'c', 'd'];
  assert.deepEqual(moveToIndex(ids, 'c', 1), ['a', 'c', 'b', 'd']);
  assert.deepEqual(moveToIndex(ids, 'a', 3), ['b', 'c', 'd', 'a']);
  assert.deepEqual(moveToIndex(ids, 'b', 0), ['b', 'a', 'c', 'd']);
  assert.deepEqual(moveToIndex(ids, 'b', 99), ['a', 'c', 'd', 'b'], '越界夹到末尾');
  assert.deepEqual(ids, ['a', 'b', 'c', 'd'], '输入不得被修改');
  assert.deepEqual(moveToIndex(ids, 'unknown', 1), ids);
});

test('sameOrder compares exact sequences', () => {
  assert.equal(sameOrder(['a', 'b'], ['a', 'b']), true);
  assert.equal(sameOrder(['a', 'b'], ['b', 'a']), false);
  assert.equal(sameOrder(['a'], ['a', 'b']), false);
});

test('device revision gate accepts the first snapshot, rejects stale and equal revisions', () => {
  const snap = (revision, ids = ['a']) => ({
    devices: ids.map((id) => device(id, id)),
    deviceList: {
      sortMode: 'manual',
      directions: { status: 'asc', name: 'asc', lastConnected: 'desc' },
      manualOrder: ids,
      revision,
    },
  });

  const first = snap(0);
  assert.equal(reduceDeviceList(null, first).accepted, true, '初次接收 revision 0');

  const second = snap(1, ['b']);
  const step2 = reduceDeviceList(first, second);
  assert.equal(step2.accepted, true);
  assert.equal(step2.next.devices[0].id, 'b');

  const stale = snap(0, ['old']);
  const rejected = reduceDeviceList(step2.next, stale);
  assert.equal(rejected.accepted, false, '旧修订号不得回退设备列表');
  assert.equal(rejected.next.devices[0].id, 'b');

  assert.equal(reduceDeviceList(step2.next, snap(1, ['same'])).accepted, false, '同修订号视为迟到');
});

test('status sequence gate requires a strictly newer sequence', () => {
  assert.equal(acceptStatusSequence(-1, 1), true);
  assert.equal(acceptStatusSequence(1, 2), true);
  assert.equal(acceptStatusSequence(2, 2), false);
  assert.equal(acceptStatusSequence(2, 1), false);
});
