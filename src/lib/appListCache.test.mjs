import test from 'node:test';
import assert from 'node:assert/strict';
import {
  APP_LIST_CACHE_TTL_MS,
  APP_SORT_CACHE_KEY,
  appListCacheKey,
  readAppSortPreference,
  readCachedApps,
  removeCachedApps,
  writeAppSortPreference,
  writeCachedApps,
} from './appListCache.ts';

const ASC_ASC = { name: 'asc', package: 'asc' };
const DESC_ASC = { name: 'desc', package: 'asc' };

const APPS = [
  { name: '设置', package: 'com.android.settings', system: true },
  { name: 'F-Droid', package: 'org.fdroid.fdroid', system: false },
];
const NOW = Date.UTC(2026, 10, 19);

function memoryStorage() {
  const map = new Map();
  return {
    map,
    getItem: (key) => (map.has(key) ? map.get(key) : null),
    setItem: (key, value) => void map.set(key, String(value)),
    removeItem: (key) => void map.delete(key),
  };
}

test('round-trips a device list through the cache', () => {
  const storage = memoryStorage();
  writeCachedApps('dev-a', APPS, NOW, storage);
  assert.deepEqual(readCachedApps('dev-a', NOW + 1000, storage), APPS);
});

test('round-trips an empty successful list', () => {
  const storage = memoryStorage();
  writeCachedApps('dev-a', [], NOW, storage);
  assert.deepEqual(readCachedApps('dev-a', NOW, storage), []);
});

test('treats a cache older than the TTL as a miss and removes it', () => {
  const storage = memoryStorage();
  writeCachedApps('dev-a', APPS, NOW - APP_LIST_CACHE_TTL_MS - 1, storage);
  assert.equal(readCachedApps('dev-a', NOW, storage), null);
  assert.equal(storage.map.has(appListCacheKey('dev-a')), false);
});

test('keeps a cache exactly at the TTL boundary', () => {
  const storage = memoryStorage();
  writeCachedApps('dev-a', APPS, NOW - APP_LIST_CACHE_TTL_MS, storage);
  assert.deepEqual(readCachedApps('dev-a', NOW, storage), APPS);
});

test('rejects an unknown cache version and removes it', () => {
  const storage = memoryStorage();
  storage.setItem(appListCacheKey('dev-a'), JSON.stringify({ v: 2, savedAt: NOW, apps: APPS }));
  assert.equal(readCachedApps('dev-a', NOW, storage), null);
  assert.equal(storage.map.has(appListCacheKey('dev-a')), false);
});

test('rejects corrupt JSON and removes it', () => {
  const storage = memoryStorage();
  storage.setItem(appListCacheKey('dev-a'), '{not json');
  assert.equal(readCachedApps('dev-a', NOW, storage), null);
  assert.equal(storage.map.has(appListCacheKey('dev-a')), false);
});

test('rejects a payload with a non-finite savedAt', () => {
  const storage = memoryStorage();
  storage.setItem(appListCacheKey('dev-a'), JSON.stringify({ v: 1, savedAt: null, apps: APPS }));
  assert.equal(readCachedApps('dev-a', NOW, storage), null);
});

test('keeps devices independent', () => {
  const storage = memoryStorage();
  writeCachedApps('dev-a', APPS, NOW, storage);
  writeCachedApps('dev-b', [APPS[1]], NOW, storage);
  removeCachedApps('dev-a', storage);
  assert.equal(readCachedApps('dev-a', NOW, storage), null);
  assert.deepEqual(readCachedApps('dev-b', NOW, storage), [APPS[1]]);
});

test('drops malformed entries but keeps valid ones', () => {
  const storage = memoryStorage();
  storage.setItem(
    appListCacheKey('dev-a'),
    JSON.stringify({
      v: 1,
      savedAt: NOW,
      apps: [APPS[0], { name: '缺包名' }, null, { name: 'X', package: 'x.y', system: 'yes' }],
    }),
  );
  assert.deepEqual(readCachedApps('dev-a', NOW, storage), [APPS[0]]);
});

test('treats an all-invalid list as corrupt', () => {
  const storage = memoryStorage();
  storage.setItem(
    appListCacheKey('dev-a'),
    JSON.stringify({ v: 1, savedAt: NOW, apps: [{ name: '', package: '', system: false }] }),
  );
  assert.equal(readCachedApps('dev-a', NOW, storage), null);
  assert.equal(storage.map.has(appListCacheKey('dev-a')), false);
});

test('degrades silently when storage throws', () => {
  const storage = {
    getItem() {
      throw new Error('denied');
    },
    setItem() {
      throw new Error('quota');
    },
    removeItem() {
      throw new Error('denied');
    },
  };
  assert.equal(readCachedApps('dev-a', NOW, storage), null);
  assert.doesNotThrow(() => writeCachedApps('dev-a', APPS, NOW, storage));
  assert.doesNotThrow(() => removeCachedApps('dev-a', storage));
});

test('returns a miss when storage is unavailable', () => {
  assert.equal(readCachedApps('dev-a', NOW, null), null);
  assert.doesNotThrow(() => writeCachedApps('dev-a', APPS, NOW, null));
  assert.doesNotThrow(() => removeCachedApps('dev-a', null));
});

test('round-trips the app sort preference', () => {
  const storage = memoryStorage();
  writeAppSortPreference('name', DESC_ASC, storage);
  assert.deepEqual(readAppSortPreference(storage), { mode: 'name', directions: DESC_ASC });
  const raw = JSON.parse(storage.map.get(APP_SORT_CACHE_KEY));
  assert.equal(raw.v, 2);
});

test('round-trips the recent sort mode', () => {
  const storage = memoryStorage();
  writeAppSortPreference('recent', ASC_ASC, storage);
  assert.deepEqual(readAppSortPreference(storage), { mode: 'recent', directions: ASC_ASC });
  const raw = JSON.parse(storage.map.get(APP_SORT_CACHE_KEY));
  assert.equal(raw.v, 2);
});

test('migrates the v1 direction into the matching mode', () => {
  const storage = memoryStorage();
  storage.setItem(APP_SORT_CACHE_KEY, JSON.stringify({ v: 1, mode: 'name', desc: true }));
  assert.deepEqual(readAppSortPreference(storage), { mode: 'name', directions: DESC_ASC });

  storage.setItem(APP_SORT_CACHE_KEY, JSON.stringify({ v: 1, mode: 'package', desc: true }));
  assert.deepEqual(readAppSortPreference(storage), {
    mode: 'package',
    directions: { name: 'asc', package: 'desc' },
  });

  storage.setItem(APP_SORT_CACHE_KEY, JSON.stringify({ v: 1, mode: 'name', desc: false }));
  assert.deepEqual(readAppSortPreference(storage), { mode: 'name', directions: ASC_ASC });
});

test('rejects an unsupported sort mode and removes it', () => {
  const storage = memoryStorage();
  storage.setItem(APP_SORT_CACHE_KEY, JSON.stringify({ v: 1, mode: 'size', desc: false }));
  assert.equal(readAppSortPreference(storage), null);
  assert.equal(storage.map.has(APP_SORT_CACHE_KEY), false);
});

test('rejects a sort preference with an invalid direction', () => {
  const storage = memoryStorage();
  storage.setItem(APP_SORT_CACHE_KEY, JSON.stringify({ v: 1, mode: 'name', desc: 'yes' }));
  assert.equal(readAppSortPreference(storage), null);
  storage.setItem(
    APP_SORT_CACHE_KEY,
    JSON.stringify({ v: 2, mode: 'name', directions: { name: 'up', package: 'asc' } }),
  );
  assert.equal(readAppSortPreference(storage), null);
  assert.equal(storage.map.has(APP_SORT_CACHE_KEY), false);
});

test('returns a sort miss for corrupt JSON or unknown version', () => {
  const storage = memoryStorage();
  storage.setItem(APP_SORT_CACHE_KEY, '{oops');
  assert.equal(readAppSortPreference(storage), null);
  storage.setItem(
    APP_SORT_CACHE_KEY,
    JSON.stringify({ v: 3, mode: 'name', directions: DESC_ASC }),
  );
  assert.equal(readAppSortPreference(storage), null);
});

test('degrades silently when storage throws for the sort preference', () => {
  const storage = {
    getItem() {
      throw new Error('denied');
    },
    setItem() {
      throw new Error('quota');
    },
    removeItem() {
      throw new Error('denied');
    },
  };
  assert.equal(readAppSortPreference(storage), null);
  assert.doesNotThrow(() => writeAppSortPreference('package', ASC_ASC, storage));
  assert.equal(readAppSortPreference(null), null);
  assert.doesNotThrow(() => writeAppSortPreference('package', ASC_ASC, null));
});
