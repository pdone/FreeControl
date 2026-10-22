// 中英字典一致性检查（任务 1.4）：
// - 两份字典的键必须完全一致；
// - 英文侧不得残留中文界面文案（自名语言选项除外）；
// - 值不得为空。
//
// 依赖：Node 内置模块 + Node 的类型剥离（`--experimental-strip-types`），
// 不引入打包器或 i18n 依赖。用法：npm run check:i18n

import { zh } from '../src/lib/i18n/zh.ts';
import { en } from '../src/lib/i18n/en.ts';

const zhKeys = Object.keys(zh).sort();
const enKeys = Object.keys(en).sort();
const has = (obj, key) => Object.prototype.hasOwnProperty.call(obj, key);

const missingInEn = zhKeys.filter((key) => !has(en, key));
const extraInEn = enKeys.filter((key) => !has(zh, key));

// 「简体中文」是语言选项的自名，英文界面里也应原样出现；技术值不进字典。
const ALLOWED_CHINESE_VALUES = new Set(['简体中文']);
const CJK = /[\u3400-\u9fff]/;
const chineseInEn = Object.entries(en).filter(
  ([, value]) => typeof value === 'string' && CJK.test(value) && !ALLOWED_CHINESE_VALUES.has(value),
);

const blank = [
  ...Object.entries(zh).map(([key, value]) => ['zh', key, value]),
  ...Object.entries(en).map(([key, value]) => ['en', key, value]),
].filter(([, , value]) => typeof value !== 'string' || value.trim() === '');

// 最小冒烟：语言字段的三个键必须存在，避免脚本在空字典上「通过」。
const required = [
  'settings.language.label',
  'settings.language.followSystem',
  'settings.language.zh',
  'settings.language.en',
];
const missingRequired = required.filter((key) => !has(zh, key) || !has(en, key));

const issues = [];
if (missingInEn.length) issues.push(`en 缺少 ${missingInEn.length} 个键：${missingInEn.join(', ')}`);
if (extraInEn.length) issues.push(`en 多出 ${extraInEn.length} 个键：${extraInEn.join(', ')}`);
if (chineseInEn.length) {
  issues.push(
    `en 含未翻译的中文（${chineseInEn.length} 处）：${chineseInEn
      .map(([key, value]) => `${key}=${value}`)
      .join(', ')}`,
  );
}
if (blank.length) {
  issues.push(`空值（${blank.length} 处）：${blank.map(([lang, key]) => `${lang}:${key}`).join(', ')}`);
}
if (missingRequired.length) issues.push(`缺少必需键：${missingRequired.join(', ')}`);

if (issues.length) {
  console.error('[check-i18n] 失败');
  for (const issue of issues) console.error(`- ${issue}`);
  process.exit(1);
}

console.log(`[check-i18n] 通过：${zhKeys.length} 个键，zh/en 键一致，en 无未翻译中文`);
