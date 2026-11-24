import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { BUILTIN_DEFAULTS } from './defaults.ts';
import { buildArgs, mergeParams } from './command.ts';

const fixture = JSON.parse(
  readFileSync(new URL('../../tests/fixtures/local-control-argv.json', import.meta.url), 'utf8'),
);

function deviceKeysOf(override) {
  if (!override || override.enabled === false) return [];
  return Object.entries(override.values ?? {}).flatMap(([group, values]) =>
    values && typeof values === 'object' && !Array.isArray(values)
      ? Object.keys(values).map((key) => `${group}.${key}`)
      : [],
  );
}

test('argv fixture base stays aligned with built-in defaults', () => {
  assert.equal(fixture.version, 1);
  assert.deepEqual(fixture.base, BUILTIN_DEFAULTS);
  assert.equal(new Set(fixture.cases.map((item) => item.name)).size, fixture.cases.length);
});

for (const item of fixture.cases) {
  test(`argv fixture: ${item.name}`, () => {
    let params = mergeParams(fixture.base, item.global);
    if (item.override && item.override.enabled !== false) {
      params = mergeParams(params, item.override.values);
    }
    const errors = Object.fromEntries(item.errors.map((key) => [key, 'invalid']));
    const actual = buildArgs(params, errors, new Set(deviceKeysOf(item.override)));
    assert.deepEqual(actual, item.expected);
  });
}
