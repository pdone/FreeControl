import test from 'node:test';
import assert from 'node:assert/strict';
import { INPUT_MODE_DEFAULTS, migrateInputModes } from './defaults.ts';

const legacyConfig = () => ({
  defaults: {
    control: {
      keyboard: 'uhid',
      mouse: 'uhid',
      gamepad: 'uhid',
      noKeyRepeat: true,
    },
    advanced: { customArgs: '--keyboard=uhid' },
  },
  overrides: {
    dev1: { enabled: true, values: { control: { keyboard: 'uhid' } } },
    dev2: { enabled: false, values: { control: { mouse: 'aoa' } } },
  },
  migrations: {},
});

test('旧 uhid 一次性归一为 scrcpy 默认值并落下标记', () => {
  const cfg = legacyConfig();
  assert.equal(migrateInputModes(cfg), true);
  assert.deepEqual(INPUT_MODE_DEFAULTS, { keyboard: 'sdk', mouse: 'sdk', gamepad: 'disabled' });
  assert.deepEqual(cfg.defaults.control, {
    keyboard: 'sdk',
    mouse: 'sdk',
    gamepad: 'disabled',
    noKeyRepeat: true,
  });
  assert.equal(cfg.overrides.dev1.values.control.keyboard, 'sdk');
  assert.equal(cfg.overrides.dev2.values.control.mouse, 'aoa', '非 uhid 的设备覆盖不改');
  assert.deepEqual(cfg.defaults.advanced, { customArgs: '--keyboard=uhid' }, '自定义参数不改');
  assert.equal(cfg.migrations.inputModeDefaults, true);
});

test('非 uhid 取值与自定义参数原样保留', () => {
  const cfg = {
    defaults: {
      control: { keyboard: 'sdk', mouse: 'aoa', gamepad: 'disabled' },
      advanced: { customArgs: '--keyboard=uhid' },
    },
    overrides: { dev1: { values: { control: { keyboard: 'disabled' } } } },
    migrations: { inputModeDefaults: true },
  };
  assert.equal(migrateInputModes(cfg), false);
  assert.deepEqual(cfg.defaults.control, {
    keyboard: 'sdk',
    mouse: 'aoa',
    gamepad: 'disabled',
  });
  assert.equal(cfg.overrides.dev1.values.control.keyboard, 'disabled');
  assert.equal(cfg.defaults.advanced.customArgs, '--keyboard=uhid');
});

test('缺字段、空对象与非法结构安全，并在首次调用后补标记', () => {
  assert.equal(migrateInputModes({}), true);
  assert.equal(migrateInputModes({ defaults: null, overrides: null, migrations: null }), true);

  const malformed = {
    defaults: { control: [] },
    overrides: { dev1: { values: { control: 'uhid' } }, dev2: null },
    migrations: {},
  };
  assert.equal(migrateInputModes(malformed), true);
  assert.deepEqual(malformed.defaults.control, []);
  assert.equal(malformed.overrides.dev1.values.control, 'uhid');
  assert.equal(malformed.migrations.inputModeDefaults, true);
});

test('归一后用户主动选择 uhid 不会被再次改写', () => {
  const cfg = { defaults: { control: { keyboard: 'sdk', mouse: 'sdk', gamepad: 'disabled' } }, migrations: {} };
  assert.equal(migrateInputModes(cfg), true);
  cfg.defaults.control.keyboard = 'uhid';
  assert.equal(migrateInputModes(cfg), false);
  assert.equal(cfg.defaults.control.keyboard, 'uhid');
});

test('标记已存在时即使取值是 uhid 也不再归一', () => {
  const cfg = {
    defaults: { control: { keyboard: 'uhid', mouse: 'uhid', gamepad: 'uhid' } },
    migrations: { inputModeDefaults: true },
  };
  assert.equal(migrateInputModes(cfg), false);
  assert.equal(cfg.defaults.control.keyboard, 'uhid');
  assert.equal(cfg.defaults.control.mouse, 'uhid');
  assert.equal(cfg.defaults.control.gamepad, 'uhid');
});
