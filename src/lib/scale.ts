// 界面缩放的应用层（spec `app-preferences` 的「界面缩放的档位与持久化」）。
//
// 缩放走 **WebView 整体缩放**（Tauri `WebviewWindow::set_zoom`），而不是给 89 处
// `font-size` 字面量各乘一个系数：字号、图标、间距与控件尺寸必须一起变，而 `.dev-alias .tag`
// 那类「行盒按字号手算」的地方经不起只放大一半（见 design D1）。
//
// 三条约束，改之前先看：
// - **同值不重复发 IPC**：配置每次变化都会走到 store 的订阅，反复设同一个缩放因子只会打断布局；
// - **页面重载后必须重新应用**：WebView 的缩放因子不随 DOM 走，reload 会把它重置为 1。
//   本应用目前没有 reload 路径，将来若加上，必须在重载后调用 `applyUiScale`；
// - **失败只记录**：缺 `core:webview:allow-set-webview-zoom` 时 promise 会被拒，界面看不出
//   任何异常、只有「改了档位没反应」—— 构建期由 Rust 侧的能力清单单测兜底。
import { getCurrentWebview } from '@tauri-apps/api/webview';
import type { UiScale } from './types';

/**
 * 首帧缩放的缓存键。
 *
 * 独立于字体的 `freecontrol-ui-fonts`：那份值是 `{ uiFont, codeFont }` 对象，
 * 混入一个标量会让已发布版本的读取逻辑落入「旧形状忽略」分支，白白丢掉字体首帧提示。
 */
export const SCALE_CACHE_KEY = 'freecontrol-ui-scale';

export const DEFAULT_UI_SCALE: UiScale = '100';

/** 四档与显示标签。标签是纯百分比，不进 i18n 字典。 */
export const SCALES: ReadonlyArray<{ value: UiScale; label: string }> = [
  { value: '100', label: '100%' },
  { value: '115', label: '115%' },
  { value: '130', label: '130%' },
  { value: '150', label: '150%' },
];

/** 档位 → WebView 缩放因子。 */
const FACTORS: Record<UiScale, number> = {
  '100': 1,
  '115': 1.15,
  '130': 1.3,
  '150': 1.5,
};

export function isUiScale(v: unknown): v is UiScale {
  return typeof v === 'string' && SCALES.some((s) => s.value === v);
}

/** 档位 → 缩放因子；未知取值按 100%（与后端 `Ui::normalize` 同一口径）。 */
export function scaleOf(v: unknown): number {
  return FACTORS[isUiScale(v) ? v : DEFAULT_UI_SCALE];
}

function readScaleCache(): UiScale | null {
  try {
    const raw = localStorage.getItem(SCALE_CACHE_KEY);
    return isUiScale(raw) ? raw : null;
  } catch {
    return null;
  }
}

/** 回写首帧缓存；存储不可用时只是下次启动没有首帧提示，不影响本次生效。 */
function writeScaleCache(scale: UiScale): void {
  try {
    localStorage.setItem(SCALE_CACHE_KEY, scale);
  } catch {
    // 忽略：缩放已生效，缓存只影响下一次启动的首帧。
  }
}

/** 已成功应用的档位；`null` 表示还没应用过（或上次失败，允许重试）。 */
let applied: UiScale | null = null;

/**
 * 应用一份缩放偏好：幂等，非浏览器环境只更新模块内状态。
 *
 * 失败时把 `applied` 复位，让下一次订阅（或下一次启动）还能再试一次 —— 权限缺失
 * 属于配置错误，不该让界面永久停在错误的缩小状态上。
 */
export async function applyUiScale(pref: unknown): Promise<void> {
  const scale = isUiScale(pref) ? pref : DEFAULT_UI_SCALE;
  if (applied === scale) return;
  applied = scale;
  writeScaleCache(scale);
  if (typeof window === 'undefined') return;
  try {
    await getCurrentWebview().setZoom(scaleOf(scale));
  } catch (e) {
    applied = null;
    console.error('应用界面缩放失败（检查 core:webview:allow-set-webview-zoom）', e);
  }
}

/**
 * 挂载前调用：用上次生效的档位兜住配置到达前的第一帧。
 *
 * 没有缓存时不发 IPC —— WebView 的默认缩放因子就是 1，多设一次只会多一轮往返。
 * 返回的 promise 由 `revealWindowWhenPainted` 在放窗口出来之前等待（带超时）。
 */
export function applyUiScaleFromCache(): Promise<void> {
  const cached = readScaleCache();
  if (!cached) {
    applied = DEFAULT_UI_SCALE;
    return Promise.resolve();
  }
  return applyUiScale(cached);
}
