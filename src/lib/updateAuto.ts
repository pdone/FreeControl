// 自动检查更新的前端调度（add-app-self-update / design D10–D11）。
//
// 只决定「何时调用检查」：检查本身仍走 store 的 `runAppUpdateCheck`（单飞），
// 发现新版本后由 `openUpdateDialog` 以预置结果打开更新视图。
//
// 频率口径（用户确认）：
// - launch：本次运行启动后查一次；
// - daily / weekly / monthly：距上次检查达到 24h / 7d / 30d 即到期；
// - 失败也算上次检查并顺延；
// - 最小化时 WebView 可能节流计时器，按天粒度可接受。

import { get } from 'svelte/store';
import {
  aboutOpen,
  appUpdateBusy,
  config,
  envGuideDismissed,
  envStatus,
  hasCheckedThisSession,
  openUpdateDialog,
  runAppUpdateCheck,
} from './store';
import { pendingInstall } from './install';
import type { AppUpdateCheck, UpdatePrefs } from './types';

const TICK_MS = 60_000;
const START_DELAY_MS = 1_200;
const DAY_MS = 24 * 60 * 60 * 1000;

const DEFAULT_UPDATE: UpdatePrefs = {
  autoCheck: false,
  frequency: 'launch',
  lastCheckedAt: null,
};

let started = false;
let ready = false;
let timer: ReturnType<typeof setInterval> | null = null;
let pendingPrompt: AppUpdateCheck | null = null;

export function startAutoUpdateChecks(): void {
  if (started) return;
  started = true;

  config.subscribe(() => {
    reschedule();
    if (ready) void maybeAutoCheck();
    flushPendingPrompt();
  });
  // 弹窗关闭后补一次判断：本次启动尚未查过、或到期时刚被弹窗挡住的情形。
  aboutOpen.subscribe((open) => {
    if (!open) {
      void maybeAutoCheck();
      flushPendingPrompt();
    }
  });
  // 首启引导 / 未校验确认关闭后再弹。
  envStatus.subscribe(() => flushPendingPrompt());
  envGuideDismissed.subscribe(() => flushPendingPrompt());
  pendingInstall.subscribe(() => flushPendingPrompt());

  setTimeout(() => {
    ready = true;
    reschedule();
    void maybeAutoCheck();
  }, START_DELAY_MS);
}

function updatePrefs(): UpdatePrefs {
  return get(config)?.update ?? DEFAULT_UPDATE;
}

function dueNow(prefs: UpdatePrefs, now: number): boolean {
  if (!prefs.autoCheck) return false;
  if (prefs.frequency === 'launch') return !hasCheckedThisSession();
  const interval =
    prefs.frequency === 'daily'
      ? DAY_MS
      : prefs.frequency === 'weekly'
        ? 7 * DAY_MS
        : 30 * DAY_MS;
  return prefs.lastCheckedAt == null || now - prefs.lastCheckedAt >= interval;
}

function reschedule(): void {
  if (timer) {
    clearInterval(timer);
    timer = null;
  }
  const prefs = updatePrefs();
  if (!prefs.autoCheck || prefs.frequency === 'launch') return;
  timer = setInterval(() => {
    void maybeAutoCheck();
  }, TICK_MS);
}

/** 可被多个订阅重复调用；到期、单飞与弹窗状态由内部判断。 */
async function maybeAutoCheck(): Promise<void> {
  if (!ready) return;
  if (!dueNow(updatePrefs(), Date.now())) return;
  // 更新弹窗已打开或更新正在下载 / 应用：本轮跳过。
  if (get(aboutOpen) || get(appUpdateBusy)) return;

  try {
    const result = await runAppUpdateCheck();
    // 检查期间可能被关掉自动检查：关掉后不弹。
    if (!updatePrefs().autoCheck) return;
    if (result.state === 'updateAvailable' || result.state === 'ready') {
      requestPrompt(result);
    }
  } catch {
    // 自动检查失败静默；技术原因由后端写日志。
  }
}

function requestPrompt(result: AppUpdateCheck): void {
  // 已有关于 / 更新弹窗：不叠层，直接丢掉这次提示。
  if (get(aboutOpen)) return;
  if (envGuideVisible() || get(pendingInstall)) {
    pendingPrompt = result;
    return;
  }
  pendingPrompt = null;
  openUpdateDialog({ mode: 'result', result });
}

function envGuideVisible(): boolean {
  const env = get(envStatus);
  return !!env && !env.ready && !get(envGuideDismissed);
}

function flushPendingPrompt(): void {
  if (!pendingPrompt) return;
  if (!updatePrefs().autoCheck) {
    pendingPrompt = null;
    return;
  }
  if (get(aboutOpen)) return;
  if (envGuideVisible() || get(pendingInstall)) return;
  const result = pendingPrompt;
  pendingPrompt = null;
  openUpdateDialog({ mode: 'result', result });
}
