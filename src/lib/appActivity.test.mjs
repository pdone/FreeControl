import test from 'node:test';
import assert from 'node:assert/strict';
import {
  APP_PINS_KEY,
  APP_USAGE_KEY,
  readAppPins,
  readAppUsage,
  withAppLaunch,
  withPin,
  withoutDevicePins,
  withoutDeviceUsage,
  writeAppPins,
  writeAppUsage,
} from './appActivity.ts';

const NOW = Date.UTC(2026, 10, 20);

function memoryStorage() {
  const map = new Map();
  return {
    map,
    getItem: (key) => (map.has(key) ? map.get(key) : null),
    setItem: (key, value) => void map.set(key, String(value)),
    removeItem: (key) => void map.delete(key),
  };
}

test('round-trips pins and drops duplicates or empty names', () => {
  const storage = memoryStorage();
  writeAppPins({ 'dev-a': ['a.one', 'a.one', '', 'b.two'] }, storage);
  assert.deepEqual(readAppPins(storage), { 'dev-a': ['a.one', 'b.two'] });
});

test('returns an empty pin table for a missing or unavailable store', () => {
  assert.deepEqual(readAppPins(memoryStorage()), {});
  assert.deepEqual(readAppPins(null), {});
});

test('drops a pin payload with an unknown version', () => {
  const storage = memoryStorage();
  storage.setItem(APP_PINS_KEY, JSON.stringify({ v: 2, devices: { 'dev-a': ['a.one'] } }));
  assert.deepEqual(readAppPins(storage), {});
  assert.equal(storage.map.has(APP_PINS_KEY), false);
});

test('drops corrupt pin JSON', () => {
  const storage = memoryStorage();
  storage.setItem(APP_PINS_KEY, '{not json');
  assert.deepEqual(readAppPins(storage), {});
  assert.equal(storage.map.has(APP_PINS_KEY), false);
});

test('withPin adds, keeps and removes without mutating the input', () => {
  const base = { 'dev-a': ['a.one'] };
  const added = withPin(base, 'dev-a', 'b.two', true);
  assert.deepEqual(added, { 'dev-a': ['a.one', 'b.two'] });
  assert.deepEqual(base, { 'dev-a': ['a.one'] });
  assert.deepEqual(withPin(added, 'dev-a', 'b.two', true), added);
  assert.deepEqual(withPin(added, 'dev-a', 'a.one', false), { 'dev-a': ['b.two'] });
  assert.deepEqual(withPin({ 'dev-a': ['a.one'] }, 'dev-a', 'a.one', false), {});
});

test('withoutDevicePins removes one device only', () => {
  const pins = { 'dev-a': ['a.one'], 'dev-b': ['b.two'] };
  assert.deepEqual(withoutDevicePins(pins, 'dev-a'), { 'dev-b': ['b.two'] });
  assert.equal(withoutDevicePins(pins, 'dev-missing'), pins);
});

test('round-trips usage records', () => {
  const storage = memoryStorage();
  const usage = { 'dev-a': { 'a.one': { lastOpenedAt: NOW, launchCount: 3 } } };
  writeAppUsage(usage, storage);
  assert.deepEqual(readAppUsage(storage), usage);
});

test('withAppLaunch increments the count and updates the timestamp', () => {
  const first = withAppLaunch({}, 'dev-a', 'a.one', NOW);
  assert.deepEqual(first, { 'dev-a': { 'a.one': { lastOpenedAt: NOW, launchCount: 1 } } });
  const second = withAppLaunch(first, 'dev-a', 'a.one', NOW + 5);
  assert.deepEqual(second['dev-a']['a.one'], { lastOpenedAt: NOW + 5, launchCount: 2 });
  assert.deepEqual(first['dev-a']['a.one'], { lastOpenedAt: NOW, launchCount: 1 });
});

test('drops invalid usage entries but keeps valid ones', () => {
  const storage = memoryStorage();
  storage.setItem(
    APP_USAGE_KEY,
    JSON.stringify({
      v: 1,
      devices: {
        'dev-a': {
          good: { lastOpenedAt: NOW, launchCount: 2 },
          badTime: { lastOpenedAt: 'yesterday', launchCount: 1 },
          badCount: { lastOpenedAt: NOW, launchCount: 1.5 },
        },
      },
    }),
  );
  assert.deepEqual(readAppUsage(storage), {
    'dev-a': { good: { lastOpenedAt: NOW, launchCount: 2 } },
  });
});

test('drops a usage payload with no valid entries', () => {
  const storage = memoryStorage();
  storage.setItem(APP_USAGE_KEY, JSON.stringify({ v: 1, devices: { 'dev-a': { bad: 1 } } }));
  assert.deepEqual(readAppUsage(storage), {});
  assert.equal(storage.map.has(APP_USAGE_KEY), false);
});

test('drops a usage payload with an unknown version', () => {
  const storage = memoryStorage();
  storage.setItem(APP_USAGE_KEY, JSON.stringify({ v: 0, devices: {} }));
  assert.deepEqual(readAppUsage(storage), {});
  assert.equal(storage.map.has(APP_USAGE_KEY), false);
});

test('returns empty usage when the store is unavailable', () => {
  assert.deepEqual(readAppUsage(null), {});
  writeAppUsage({ 'dev-a': { 'a.one': { lastOpenedAt: NOW, launchCount: 1 } } }, null);
  assert.deepEqual(readAppUsage(null), {});
});

test('withoutDeviceUsage removes one device only', () => {
  const usage = {
    'dev-a': { 'a.one': { lastOpenedAt: NOW, launchCount: 1 } },
    'dev-b': { 'b.two': { lastOpenedAt: NOW, launchCount: 1 } },
  };
  assert.deepEqual(withoutDeviceUsage(usage, 'dev-a'), {
    'dev-b': { 'b.two': { lastOpenedAt: NOW, launchCount: 1 } },
  });
  assert.equal(withoutDeviceUsage(usage, 'dev-missing'), usage);
});
