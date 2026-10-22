import type { Lang, LanguagePref } from '../types';

function normalizeLang(raw: string | null | undefined): Lang | null {
  if (!raw) return null;
  const lower = raw.trim().toLowerCase();
  if (lower.startsWith('zh')) return 'zh';
  if (lower.startsWith('en')) return 'en';
  return null;
}

/** 单个语言标签 → 支持的语言；不认识的一律回退英文。 */
export function languageFrom(raw: string | null | undefined): Lang {
  return normalizeLang(raw) ?? 'en';
}

/**
 * 按系统首选语言解析生效语言。
 *
 * 只取首选一项：`fr-FR, zh-CN` 的系统仍按不支持的 `fr-FR` 回退英文，
 * 不从次选语言里挑一个「支持的语言」—— 那会让界面语言与用户实际首选不符。
 */
export function resolveSystemLanguage(): Lang {
  if (typeof navigator === 'undefined') return 'en';
  const primary = navigator.languages?.[0] ?? navigator.language;
  return languageFrom(primary);
}

export function isLang(value: unknown): value is Lang {
  return value === 'zh' || value === 'en';
}

export function isLanguagePref(value: unknown): value is LanguagePref {
  return value === 'system' || isLang(value);
}

export function langTag(lang: Lang): 'zh-CN' | 'en' {
  return lang === 'zh' ? 'zh-CN' : 'en';
}
