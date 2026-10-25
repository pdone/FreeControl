// 界面字体与代码字体的应用层（spec `app-preferences` 的「所选字体与系统默认回退」）。
//
// 两个变量各自只有一条栈：`--font-sans` 以界面字体开头、系统默认无衬线栈结尾；
// `--font-mono` 以代码字体开头、系统默认等宽栈结尾。逐字形回退完全交给内核，
// 不按语言或字符类型切分文本。默认栈由 `app.css` 的 `--font-*-default` 单点定义。
//
// 另有一份 localStorage 首帧缓存：主窗口在首帧后才显示、配置是异步读取的，
// 若不缓存，每次启动都会看到一次字体切换。缓存只是提示，配置到达后立即覆盖。
import type { SystemFont } from './types';

/** 首帧字体缓存的键；值形如 `{ uiFont, codeFont }`，旧的多选数组形状会被忽略。 */
export const FONT_CACHE_KEY = 'freecontrol2-ui-fonts';

export const DEFAULT_SANS_VAR = 'var(--font-sans-default)';
export const DEFAULT_MONO_VAR = 'var(--font-mono-default)';

const CONTROL_RE = /[\u0000-\u001f\u007f]/;

/** 两个字体字段的取值；`null` 表示系统默认。 */
export interface FontChoices {
  uiFont: string | null;
  codeFont: string | null;
}

/** CSS 字符串转义：反斜杠与双引号。族名一律加引号，避免被当成 `inherit` 这类关键字。 */
function cssString(name: string): string {
  return `"${name.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`;
}

/** 组合单个区域的 `font-family`；空值返回空串（调用方据此移除行内变量）。 */
export function buildFontStack(family: string | null, fallbackVar: string): string {
  const name = family?.trim() ?? '';
  if (!name || CONTROL_RE.test(name)) return '';
  return `${cssString(name)}, ${fallbackVar}`;
}

function applyVariable(name: string, value: string): void {
  const root = document.documentElement;
  if (value) root.style.setProperty(name, value);
  else root.style.removeProperty(name);
}

/** 把界面字体 / 代码字体分别写到 `<html>` 的行内变量；空值移除行内值。 */
export function applyFontStacks(choices: FontChoices): void {
  if (typeof document === 'undefined') return;
  applyVariable('--font-sans', buildFontStack(choices.uiFont, DEFAULT_SANS_VAR));
  applyVariable('--font-mono', buildFontStack(choices.codeFont, DEFAULT_MONO_VAR));
}

function readFontCache(): FontChoices | null {
  try {
    const raw = localStorage.getItem(FONT_CACHE_KEY);
    if (!raw) return null;
    const parsed: unknown = JSON.parse(raw);
    // 旧的多选数组（或多选对象）不是本形状，直接忽略。
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return null;
    const record = parsed as Record<string, unknown>;
    return {
      uiFont: typeof record.uiFont === 'string' ? record.uiFont : null,
      codeFont: typeof record.codeFont === 'string' ? record.codeFont : null,
    };
  } catch {
    return null;
  }
}

/** 回写首帧缓存；存储不可用时只是下次启动没有首帧提示，不影响本次生效。 */
export function writeFontCache(choices: FontChoices): void {
  try {
    localStorage.setItem(
      FONT_CACHE_KEY,
      JSON.stringify({ uiFont: choices.uiFont, codeFont: choices.codeFont }),
    );
  } catch {
    // 忽略：字体仍会由配置在本次运行中应用。
  }
}

/** 挂载前调用：用上次的两个字体兜住配置到达前的第一帧。 */
export function initFontStacks(): void {
  const cached = readFontCache();
  if (cached) applyFontStacks(cached);
}

/**
 * 选择一个内核能解析的族名。
 *
 * fontdb / DirectWrite 给的是字体族名，绝大多数情况下与 WebView 内核的解析名一致；
 * 若 canonical 名解析不到（例如某些本地化命名差异），改用第一个能解析的别名。
 * 全部解析不到时仍返回 canonical —— 宁可让 CSS 自己回退，也不在列表里藏起字体。
 */
export function resolveCssFamily(font: SystemFont): string {
  if (
    typeof document === 'undefined' ||
    typeof document.fonts?.check !== 'function'
  ) {
    return font.family;
  }
  for (const name of [font.family, ...font.aliases]) {
    try {
      if (document.fonts.check(`14px ${cssString(name)}`)) return name;
    } catch {
      // 单个名字解析失败不影响其它候选。
    }
  }
  return font.family;
}

/** 当前界面语言下的显示名：中文优先含汉字的别名，英文用 canonical。 */
export function displayFamily(font: SystemFont, lang: 'zh' | 'en'): string {
  if (lang === 'zh') {
    const localized = font.aliases.find((name) => /[\u3400-\u9fff]/.test(name));
    if (localized) return localized;
  }
  return font.family;
}

/** 搜索匹配：canonical 与全部本地化别名都参与，不区分大小写。 */
export function fontMatches(font: SystemFont, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return [font.family, ...font.aliases].some((name) =>
    name.toLowerCase().includes(q),
  );
}
