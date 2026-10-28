// 深浅外观的取值、持久化与切换。
//
// 站点只有两档（深 / 浅），**没有**「跟随系统」：系统颜色模式只在"没有保存过选择"时决定首帧，
// 读者按下过一次之后，系统怎么变都与页面无关 —— 因此这里不需要 media 变化监听。
//
// 切换带一层整页交叉淡入（与产品同值），由 `commitTheme` 调 View Transitions；时长与缓动在
// app.css 的 `::view-transition-*` 里定，减少动态效果时直接写 DOM。
//
// 纯函数（`isThemeMode` / `otherTheme` / `initialMode`）与 DOM 解耦，便于按本仓既有做法用
// esbuild 打包后直接核对；写 DOM 的动作只在 `applyTheme` 一处。
//
// 存储键与 index.html 首帧脚本里的那一个是同一个 —— 那里必须在样式表前**同步**读一次，
// 二者的取值域（`light` / `dark`）与降级口径（存储不可用就按系统）须保持一致。
import { get, writable } from 'svelte/store';

export type ThemeMode = 'light' | 'dark';

const STORAGE_KEY = 'freecontrol2-website-theme';

/** 与 tokens.css 两档的 `--bg` 同值：浏览器用它给页面**自身之外**的区域着色（手机地址栏）。 */
const THEME_COLOR: Record<ThemeMode, string> = { dark: '#0b0d11', light: '#f2f4f7' };

export function isThemeMode(v: unknown): v is ThemeMode {
  return v === 'light' || v === 'dark';
}

/** 另一档：切换就是取它。 */
export function otherTheme(mode: ThemeMode): ThemeMode {
  return mode === 'light' ? 'dark' : 'light';
}

/**
 * 首帧档位：站内选过就用存的，否则按系统。
 *
 * 只接受 `light` / `dark` 两个取值 —— 早期版本存过的 `system`（那一档已撤掉）一律按"没有
 * 保存过选择"处理，回落到系统判定，页面不会停在一个已经不存在的档上。
 */
export function initialMode(saved: string | null, prefersDark: boolean): ThemeMode {
  if (isThemeMode(saved)) return saved;
  return prefersDark ? 'dark' : 'light';
}

function readSaved(): string | null {
  try {
    return localStorage.getItem(STORAGE_KEY);
  } catch {
    // 存储不可用：按系统判定，切换仍在本页生效
    return null;
  }
}

function systemPrefersDark(): boolean {
  return typeof window !== 'undefined' && window.matchMedia('(prefers-color-scheme: dark)').matches;
}

function prefersReducedMotion(): boolean {
  return typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

export const themeMode = writable<ThemeMode>(initialMode(readSaved(), systemPrefersDark()));

/** 把档位写进 DOM。只写这两个位置：两档 token 由 tokens.css 的两块承接。 */
export function applyTheme(mode: ThemeMode): void {
  if (typeof document === 'undefined') return;
  document.documentElement.dataset.theme = mode;
  const meta = document.querySelector('meta[name="theme-color"]');
  if (meta) meta.setAttribute('content', THEME_COLOR[mode]);
}

/**
 * 捕获"新画面"之前要等的事（本站只有一件：让跨文档的 hero 预览先换肤）。
 *
 * 由宿主（App.svelte）注册，默认什么都不等。为什么必须在**过渡回调里**等：View Transitions
 * 的旧画面拍在回调之前、新画面拍在回调的 promise 落地之后 —— 只有让预览的换肤落在这一窗里，
 * 它才会被那一次交叉淡入一起带过去；否则预览要么在过渡开始前就跳（"先跳再淡"），要么拖到
 * 淡入结束才跳（"淡完再跳"）。两档的时长与缓动在 app.css 里与产品同值，别在这里另起一套。
 */
let beforeCapture: (mode: ThemeMode) => Promise<void> = async () => {};

export function setThemeCaptureHook(fn: (mode: ThemeMode) => Promise<void>): void {
  beforeCapture = fn;
}

/**
 * 应用档位：`animate` 为真时包一层整页交叉淡入。
 *
 * - 减少动态效果、或内核没有 View Transitions → 直接写 DOM（预览照旧被通知，只是不等回执）；
 * - 连点两下时前一段过渡被自动跳过，`finished` 的 rejection 不需要处理。
 */
function commitTheme(mode: ThemeMode, animate: boolean): void {
  if (!animate || prefersReducedMotion() || typeof document.startViewTransition !== 'function') {
    applyTheme(mode);
    void beforeCapture(mode);
    return;
  }
  document
    .startViewTransition(async () => {
      applyTheme(mode);
      await beforeCapture(mode);
    })
    .finished.catch(() => {});
}

/**
 * 设定档位并落盘。
 *
 * 只在**读者手动切换**时写存储；首帧按系统判定出来的那一档不落盘 —— 这样"尚未做过选择"
 * 这一状态不会被一次访问悄悄变成"已选择"，下次访问仍跟着系统走。
 */
export function setThemeMode(mode: ThemeMode, animate = true): void {
  if (!isThemeMode(mode) || get(themeMode) === mode) return;
  themeMode.set(mode);
  commitTheme(mode, animate);
  try {
    localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    // 存储不可用就不记忆，切换仍在本页生效
  }
}

// 首帧的 data-theme 由 index.html 的同步脚本写（早于样式表与挂载）；这里再写一次幂等兜底，
// 覆盖"内联脚本被拦"的情形。
if (typeof document !== 'undefined') applyTheme(get(themeMode));
