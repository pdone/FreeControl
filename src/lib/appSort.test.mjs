import test from 'node:test';
import assert from 'node:assert/strict';
import { sortAppEntries } from './appSort.ts';

const ASC_ASC = { name: 'asc', package: 'asc' };
const DESC_ASC = { name: 'desc', package: 'asc' };

const APPS = [
  { name: 'Alpha', package: 'com.delta', system: true },
  { name: 'Bravo', package: 'com.charlie', system: true },
  { name: 'Charlie', package: 'com.bravo', system: false },
  { name: 'Delta', package: 'com.alpha', system: false },
];

const pkgOrder = (list) => list.map((a) => a.package);

test('default order keeps the read order when nothing is pinned', () => {
  const out = sortAppEntries(APPS, 'default', ASC_ASC, 'en', new Set(), {});
  assert.deepEqual(pkgOrder(out), pkgOrder(APPS));
});

test('pinned entries always lead and keep the read order under default sort', () => {
  const out = sortAppEntries(
    APPS,
    'default',
    ASC_ASC,
    'en',
    new Set(['com.bravo', 'com.charlie']),
    {},
  );
  assert.deepEqual(pkgOrder(out), ['com.charlie', 'com.bravo', 'com.delta', 'com.alpha']);
});

test('name sort applies to each group and keeps the pinned group first', () => {
  const out = sortAppEntries(APPS, 'name', ASC_ASC, 'en', new Set(['com.delta']), {});
  assert.deepEqual(pkgOrder(out), ['com.delta', 'com.charlie', 'com.bravo', 'com.alpha']);
  const desc = sortAppEntries(APPS, 'name', DESC_ASC, 'en', new Set(['com.delta']), {});
  assert.deepEqual(pkgOrder(desc), ['com.delta', 'com.alpha', 'com.bravo', 'com.charlie']);
});

test('package sort uses the package key inside each group', () => {
  const out = sortAppEntries(APPS, 'package', ASC_ASC, 'en', new Set(), {});
  assert.deepEqual(pkgOrder(out), ['com.alpha', 'com.bravo', 'com.charlie', 'com.delta']);
});

test('recent puts opened apps newest-first and never-opened apps after them', () => {
  const usage = {
    'com.charlie': { lastOpenedAt: 100, launchCount: 1 },
    'com.alpha': { lastOpenedAt: 300, launchCount: 2 },
  };
  const out = sortAppEntries(APPS, 'recent', ASC_ASC, 'en', new Set(), usage);
  assert.deepEqual(pkgOrder(out), ['com.alpha', 'com.charlie', 'com.delta', 'com.bravo']);
});

test('recent keeps equal timestamps in the read order', () => {
  const usage = {
    'com.delta': { lastOpenedAt: 100, launchCount: 1 },
    'com.alpha': { lastOpenedAt: 100, launchCount: 1 },
  };
  const out = sortAppEntries(APPS, 'recent', ASC_ASC, 'en', new Set(), usage);
  assert.deepEqual(pkgOrder(out), ['com.delta', 'com.alpha', 'com.charlie', 'com.bravo']);
});

test('recent keeps the pinned group first and sorts it by its own timestamps', () => {
  const usage = {
    'com.delta': { lastOpenedAt: 100, launchCount: 1 },
    'com.bravo': { lastOpenedAt: 500, launchCount: 3 },
    'com.charlie': { lastOpenedAt: 900, launchCount: 1 },
  };
  const out = sortAppEntries(
    APPS,
    'recent',
    ASC_ASC,
    'en',
    new Set(['com.delta', 'com.bravo']),
    usage,
  );
  assert.deepEqual(pkgOrder(out), ['com.bravo', 'com.delta', 'com.charlie', 'com.alpha']);
});

test('does not mutate the input list', () => {
  const before = pkgOrder(APPS);
  sortAppEntries(APPS, 'name', ASC_ASC, 'en', new Set(['com.delta']), {});
  assert.deepEqual(pkgOrder(APPS), before);
});
