// 外观（主题模式 / 主题色）的应用层。
//
// 三件事：
// - 决定**生效主题**：`system` 取系统深浅色，`dark` / `light` 固定；
// - 把解析结果写到 `<html data-theme>`，由 `app.css` 的两套 token 承接；
// - 把主题色写到 `--accent`，并按亮度决定 `--accent-ink`（压在实底上的文字色）。
//
// 这里只写这两个变量：强调色系的其余取值留在 CSS 里按主题派生（见 `app.css` 的
// `:root[data-theme='light']` 那一套），因此换颜色不会漏掉任何一处组件样式。
// 纯函数（`normalizeHex` / `relativeLuminance` / `inkFor` / `resolveTheme`）与 DOM 解耦，
// 便于按本仓既有做法用 esbuild 打包后直接核对。
import type { UiPrefs } from './types';

export type ThemeMode = UiPrefs['theme'];
export type ResolvedTheme = 'dark' | 'light';

/**
 * 内置预设（2026-10-19 用户选定）：出厂默认 indigo 保持不变，其余为 Flat UI 色系。
 *
 * 这批颜色的亮度差得较大，实底文字不能固定白色 —— 由 `inkFor` 逐色取白/黑：
 * indigo 与紫水晶用白字，其余偏亮的色（绿松石 / 翡翠 / 蓝 / 黄 / 橙 / 红）用黑字。
 * 「向日葵黄 / 胡萝卜橙」与 `--warn` 的琥珀语义相近，但这是用户明确选定的取值；
 * 警示态仍靠文案与图标区分，不单靠颜色。
 */
export const ACCENTS: ReadonlyArray<{ value: string; label: string }> = [
  { value: '#6366f1', label: '靛蓝' },
  { value: '#1abc9c', label: '绿松石色' },
  { value: '#2ecc71', label: '翡翠绿' },
  { value: '#3498db', label: '彼得河蓝' },
  { value: '#9b59b6', label: '紫水晶色' },
  { value: '#f1c40f', label: '向日葵黄' },
  { value: '#e67e22', label: '胡萝卜橙' },
  { value: '#e74c3c', label: '茜草红' },
];

export const DEFAULT_THEME: ThemeMode = 'system';
/** 出厂默认主题色（与后端 `config::DEFAULT_ACCENT`、`app.css` 的 `--accent` 同一份取值）。 */
export const DEFAULT_ACCENT = '#6366f1';

/** 生效主题的亮度阈值：`L ≤ 0.22` 取白字，否则取黑字。 */
const INK_THRESHOLD = 0.22;

const THEMES: readonly ThemeMode[] = ['system', 'dark', 'light'];

export function isThemeMode(v: unknown): v is ThemeMode {
  return typeof v === 'string' && (THEMES as readonly string[]).includes(v);
}

/**
 * 合法 `#RRGGBB` → 小写；其它形状返回 `null`。
 * 与后端 `config::normalize_hex` 同一口径（不接受短写），保证「界面判定合法」与
 * 「落盘后被后端接受」是同一件事。
 */
export function normalizeHex(raw: string): string | null {
  const s = raw.trim();
  if (!/^#[0-9a-fA-F]{6}$/.test(s)) return null;
  return s.toLowerCase();
}

export function hexToRgb(hex: string): [number, number, number] {
  return [
    parseInt(hex.slice(1, 3), 16),
    parseInt(hex.slice(3, 5), 16),
    parseInt(hex.slice(5, 7), 16),
  ];
}

/** 三个分量 → 小写 `#rrggbb`。调用方负责先做 0–255 校验。 */
export function rgbToHex(r: number, g: number, b: number): string {
  return (
    '#' +
    [r, g, b]
      .map((v) => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, '0'))
      .join('')
  );
}

/** WCAG 相对亮度。 */
export function relativeLuminance(hex: string): number {
  const [r, g, b] = hexToRgb(hex).map((v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/**
 * 压在主题色实底上的文字色。
 *
 * 按相对亮度走阈值，而**不是**逐色比较「白/黑谁的对比度更高」：indigo #6366f1 的黑字
 * （4.68）虽微胜白字（4.49），但与既有主按钮 / 分段选中项的观感相反。阈值口径与产品
 * 既有注释里的「翻转阈值 ≈0.20」一脉相承。
 */
export function inkFor(hex: string): '#fff' | '#000' {
  return relativeLuminance(hex) <= INK_THRESHOLD ? '#fff' : '#000';
}

export function resolveTheme(mode: ThemeMode, prefersLight: boolean): ResolvedTheme {
  if (mode === 'system') return prefersLight ? 'light' : 'dark';
  return mode;
}

/* ---------- 强调色派生（按对比度兜底） ----------
   `--accent-hover` / `--accent-text` 不只是装饰：它们承载导航选中文字、tag 文字、
   code 文字与图标。固定的 `color-mix` 比例对深色主题够用，但浅色主题下绿色 / 黄色 /
   青色这类高亮度主题色压不出足够的对比度（实测最低只有 2.5:1）。
   故：先按既有比例派生（保持已验收的观感），再用 `ensureContrast` 逼近目标色 ——
   只有不达标时才继续向白（深色主题）或黑（浅色主题）推，达标即停。
   目标：文字 4.5:1（WCAG AA 正文），图标同样按 4.5 从严。 */

type Rgb = [number, number, number];

/** 浅色主题正文承载面（tag 在卡片白底、导航选中在侧栏底、code 在自己的底上）。 */
const LIGHT_SURFACES: { soft: Rgb; code: Rgb; panel: Rgb } = {
  // 12% accent 会叠在下面，故 soft 要在运行时按 accent 合成（见 derivedColors）
  soft: [0xf8, 0xf9, 0xfb],
  // 与 app.css 的 --code-bg 同值：mix(--text 7%, --panel-2)
  code: [0xe2, 0xe5, 0xe8],
  panel: [0xff, 0xff, 0xff],
};
const DARK_SURFACES: { soft: Rgb; code: Rgb; panel: Rgb } = {
  soft: [0x14, 0x17, 0x1d],
  // mix(--text 9%, --panel-2) ≈ #2d323c
  code: [0x2d, 0x32, 0x3c],
  panel: [0x17, 0x1b, 0x22],
};

function mixRgb(a: Rgb, b: Rgb, wa: number): Rgb {
  return [a[0] * wa + b[0] * (1 - wa), a[1] * wa + b[1] * (1 - wa), a[2] * wa + b[2] * (1 - wa)];
}

function overRgb(fg: Rgb, alpha: number, bg: Rgb): Rgb {
  return [fg[0] * alpha + bg[0] * (1 - alpha), fg[1] * alpha + bg[1] * (1 - alpha), fg[2] * alpha + bg[2] * (1 - alpha)];
}

function contrastRgb(a: Rgb, b: Rgb): number {
  const lum = (rgb: Rgb): number => {
    const c = rgb.map((v) => {
      const x = v / 255;
      return x <= 0.03928 ? x / 12.92 : Math.pow((x + 0.055) / 1.055, 2.4);
    });
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
  };
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

function rgbCss(rgb: Rgb): string {
  return `rgb(${rgb.map((v) => Math.round(v)).join(', ')})`;
}

/** 在多个承载面里取最低对比度 —— 派生色必须每一面都达标。 */
function minContrast(rgb: Rgb, bgs: Rgb[]): number {
  return Math.min(...bgs.map((bg) => contrastRgb(rgb, bg)));
}

/** 从 base 向 toward（白或黑）推，直到所有承载面都达到 target；已达标原样返回。 */
function ensureContrast(base: Rgb, bgs: Rgb[], target: number, toward: Rgb): Rgb {
  if (minContrast(base, bgs) >= target) return base;
  let lo = 0;
  let hi = 1;
  for (let i = 0; i < 24; i++) {
    const t = (lo + hi) / 2;
    const candidate = mixRgb(base, toward, 1 - t);
    if (minContrast(candidate, bgs) >= target) hi = t;
    else lo = t;
  }
  return mixRgb(base, toward, 1 - hi);
}

/**
 * 由主题色算出生效主题下的两个派生色。
 *
 * - `hover`：图标与「压在面上的高亮文字」，深色主题向白推、浅色主题向黑推；
 * - `text`：搭柔底 / code 底的文字，同上但基础比例更浅（深色）/ 更深（浅色）。
 *
 * 基础比例与 `app.css` 的 `:root` / `:root[data-theme='light']` 保持同值：
 * 深色 hover 85% / text 35%，浅色 hover 82% / text 72%。
 */
function derivedColors(accent: Rgb, theme: ResolvedTheme): { hover: string; text: string } {
  const dark = theme === 'dark';
  const toward: Rgb = dark ? [255, 255, 255] : [0, 0, 0];
  const surfaces = dark ? DARK_SURFACES : LIGHT_SURFACES;
  // 柔底是该主题色自己的 12% / 14% 叠在承载面上 —— 对文字来说它是最不利的一面之一。
  const soft = overRgb(accent, dark ? 0.14 : 0.12, surfaces.soft);
  const hoverBg = [soft, surfaces.panel];
  const textBg = [soft, surfaces.code, surfaces.panel];

  const hoverBase = dark ? mixRgb(accent, toward, 0.85) : mixRgb(accent, toward, 0.82);
  const textBase = dark ? mixRgb(accent, toward, 0.35) : mixRgb(accent, toward, 0.72);
  // 目标取 4.6 而非 4.5：最终值会取整到整数 RGB，留一点余量避免舍入后跌破验收线。
  return {
    hover: rgbCss(ensureContrast(hoverBase, hoverBg, 4.6, toward)),
    text: rgbCss(ensureContrast(textBase, textBg, 4.6, toward)),
  };
}

let currentMode: ThemeMode = DEFAULT_THEME;
let media: MediaQueryList | null = null;

function prefersLight(): boolean {
  if (!media && typeof window !== 'undefined') {
    media = window.matchMedia('(prefers-color-scheme: light)');
  }
  return media?.matches ?? false;
}

/** 「减少动态效果」：命中时主题切换直接写 DOM，不进整页过渡。 */
function prefersReducedMotion(): boolean {
  return typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

/** 是否已应用过一次外观、上一次写入的生效主题。首帧不包过渡，避免启动时闪一下。 */
let appliedOnce = false;
let lastResolvedTheme: ResolvedTheme | null = null;

function writeAppearance(resolved: ResolvedTheme, accent: string): void {
  const root = document.documentElement;
  root.dataset.theme = resolved;
  root.style.setProperty('--accent', accent);
  root.style.setProperty('--accent-ink', inkFor(accent));
  // 两个派生色按当前主题的承载面兜底到 4.5:1（见 derivedColors 的说明）。
  const derived = derivedColors(hexToRgb(accent), resolved);
  root.style.setProperty('--accent-hover', derived.hover);
  root.style.setProperty('--accent-text', derived.text);
}

/**
 * 应用一份外观偏好：解析主题、写 `data-theme`、写 `--accent` 与 `--accent-ink`。
 *
 * 幂等、无副作用累积；配置每次变化调一次即可。非浏览器环境（如纯函数核对脚本）
 * 只更新模块内状态，不碰 DOM。
 *
 * 只有**生效主题**变化时才包一层整页交叉淡入（时长在 `app.css` 的 `::view-transition-*`
 * 里定）：主题色单独变化仍是即时生效，否则每次点色板都要等过渡。
 */
export function applyAppearance(ui: UiPrefs): void {
  currentMode = isThemeMode(ui.theme) ? ui.theme : DEFAULT_THEME;
  const accent = normalizeHex(ui.accent) ?? DEFAULT_ACCENT;
  const resolved = resolveTheme(currentMode, prefersLight());

  if (typeof document === 'undefined') {
    lastResolvedTheme = resolved;
    appliedOnce = true;
    return;
  }

  const themeChanged = appliedOnce && lastResolvedTheme !== resolved;
  appliedOnce = true;
  lastResolvedTheme = resolved;

  if (
    themeChanged &&
    typeof document.startViewTransition === 'function' &&
    !prefersReducedMotion()
  ) {
    // 连点两下主题时前一段过渡被自动跳过，abort 的 rejection 不需要处理。
    document.startViewTransition(() => writeAppearance(resolved, accent)).finished.catch(() => {});
  } else {
    writeAppearance(resolved, accent);
  }
}

/**
 * 监听系统深浅色变化：只在当前模式为 `system` 时重新解析并应用。
 *
 * `getUi` 返回当前偏好（由 store 提供），避免模块自己缓存一份配置副本。
 * 重复调用会重复挂监听 —— 只在启动时调用一次（见 `store.ts`）。
 */
export function installSystemThemeWatcher(getUi: () => UiPrefs): void {
  if (typeof window === 'undefined') return;
  media = window.matchMedia('(prefers-color-scheme: light)');
  media.addEventListener('change', () => {
    if (currentMode === 'system') applyAppearance(getUi());
  });
}
