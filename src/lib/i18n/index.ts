import { derived, get, writable } from 'svelte/store';
import type { Lang, LanguagePref } from '../types';
import { en } from './en';
import { isLang, langTag, resolveSystemLanguage } from './resolve';
import { zh, type MessageKey, type MessageParams } from './zh';

export type { Lang, LanguagePref, MessageKey, MessageParams };
export { en, zh };
export {
  isLang,
  isLanguagePref,
  langTag,
  languageFrom,
  resolveSystemLanguage,
} from './resolve';

export const DEFAULT_LANGUAGE: LanguagePref = 'system';

/** 首帧语言快照的键：只存 `zh` / `en`，不存 `system`。 */
export const LANG_CACHE_KEY = 'freecontrol-ui-lang';

function readCachedLang(): Lang | null {
  try {
    const raw = localStorage.getItem(LANG_CACHE_KEY);
    return isLang(raw) ? raw : null;
  } catch {
    return null;
  }
}

function writeCachedLang(lang: Lang): void {
  try {
    localStorage.setItem(LANG_CACHE_KEY, lang);
  } catch {
    // 存储不可用时只是下次启动没有首帧快照，不影响本次切换。
  }
}

function setDocumentLang(lang: Lang): void {
  if (typeof document !== 'undefined') document.documentElement.lang = langTag(lang);
}

const initialLang: Lang = readCachedLang() ?? resolveSystemLanguage();

/** 当前生效语言。首帧取本机快照；配置到达后由 store 的订阅覆盖。 */
export const resolvedLang = writable<Lang>(initialLang);

/** 把生效语言写到根元素与首帧快照；幂等。 */
function syncLanguage(lang: Lang): void {
  setDocumentLang(lang);
  writeCachedLang(lang);
}

// `<html lang>` 与首帧快照都跟着生效语言走：无论语言是首帧解析出来的、还是配置到达后
// 覆盖的，都只有这一个同步点 —— 不必每个调用方记得各写一遍。
resolvedLang.subscribe(syncLanguage);

function interpolate(template: string, params?: MessageParams): string {
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (whole, name: string) => {
    const value = params[name];
    return value === undefined ? whole : String(value);
  });
}

export function translate(lang: Lang, key: MessageKey, params?: MessageParams): string {
  const dict = lang === 'en' ? en : zh;
  const template = dict[key] ?? key;
  return interpolate(template, params);
}

/** 组件内的响应式翻译：`$t('key')` / `$t('key', { n })`。 */
export const t = derived(
  resolvedLang,
  ($lang) => (key: MessageKey, params?: MessageParams) => translate($lang, key, params),
);

/** 非响应式上下文（事件处理、错误兜底）用的即时翻译。 */
export function translateNow(key: MessageKey, params?: MessageParams): string {
  return translate(get(resolvedLang), key, params);
}

/**
 * 后端 code 的即时翻译：字典尚未收录该 code 时原样返回 code，
 * 由调用方决定是否再回退 `detail`。字典补齐后同一调用自动出文案。
 */
export function translateCode(code: string, params?: MessageParams): string {
  return translate(get(resolvedLang), code as MessageKey, params);
}

/**
 * 应用一档语言偏好：`system` 按系统首选解析，固定档直接用。
 * 写 `resolvedLang` 即完成 `<html lang>` 与首帧快照的同步（见上面的订阅）。
 */
export function applyLanguage(pref: LanguagePref): Lang {
  const lang = pref === 'system' ? resolveSystemLanguage() : pref;
  resolvedLang.set(lang);
  return lang;
}

/**
 * 挂载前调用：确保首帧语言已写到根元素与快照。
 *
 * 订阅在模块加载时已用首帧取值跑过一次，这里再同步一次是幂等的 —— 保留调用点是为了
 * 让 `main.ts` 的首帧时序读得出来，而不是依赖「import 的副作用」。
 */
export function initLanguage(): void {
  syncLanguage(get(resolvedLang));
}
