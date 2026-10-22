// 运行环境获取的前端状态机（tasks 4.1–4.9）。
//
// 首启缺失引导如今只做告知与转场，获取流程全发生在「运行环境」页的运行环境卡片内 ——
// 因此这套状态机也是**单点维护**：进度、当前通道、失败原因、「未经官方校验」的待确认
// 中间态只在这里维护，不存在「引导里在下载、卡片里显示未在下载」这类不一致。
//
// 流程（后端负责通道降级 / 校验 / 原子替换，前端只驱动与展示）：
//   runInstall(mode)
//     → status = done             ：安装完成，刷新环境
//     → status = needsConfirmation：无法获取官方校验值，置 pendingInstall，等用户确认
//     → status = canceled         ：用户点了「取消」，收起进度块且**不报错**
//   confirmPendingInstall() 继续（复用已下载文件，不重复下载）
//   cancelPendingInstall()  放弃（后端清掉临时文件，现有环境零改动）
//   switchChannel()         切换通道（后端中断当前尝试、改用下一条）
//   cancelInstall()         取消本次获取（在途的 runInstall 会以 canceled 收尾）
//
// 进度文案由结构化字段 + 当前语言派生，不再由后端下发成品文本；切换语言时进度会一起重译。

import { derived, writable } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import {
  startScrcpyInstall,
  confirmScrcpyInstall,
  cancelScrcpyInstall,
  switchScrcpyChannel,
} from './api';
import { envStatus, refreshEnv } from './store';
import { t } from './i18n';
import type {
  ChannelView,
  InstallPhase,
  InstallProgress,
  InstallOutcome,
  UserMessage,
} from './types';

/** 是否有安装在途（按钮禁用与进度展示共用）。 */
export const installRunning = writable<boolean>(false);
/** 当前阶段：download | verify | extract | replace | done。 */
export const installPhase = writable<InstallPhase | ''>('');
/** 是否在等待用户确认「未经官方校验仍继续」。 */
export const installAwaitingConfirmation = writable<boolean>(false);
/** 当前资产文件名。 */
export const installAsset = writable<string | null>(null);
/** 已下载 / 已解压的字节数或条目数。 */
export const installDone = writable<number | null>(null);
/** 总字节数或总条目数。 */
export const installTotal = writable<number | null>(null);
/** 安装完成时的版本号。 */
export const installVersion = writable<string | null>(null);
/** 替换阶段是否先结束了 adb 服务端。 */
export const installReplaceStoppingAdb = writable<boolean | null>(null);
/**
 * 当前实际生效的下载通道 —— 只在某条通道**已开始返回数据**（带通道名的进度事件）时被写入；
 * 一旦新一次尝试开始就被清空（见 `ensureListener`），故它不表示「上一条通道曾经成功过」，
 * 只表示「此刻正在传输的是哪条通道」。
 */
export const installChannel = writable<ChannelView | null>(null);
/** 本次尝试的序号（从 1 开始）；0 表示尚无尝试信息。 */
export const installAttempt = writable<number>(0);
/** 本次可用通道总数；0 表示尚无尝试信息。 */
export const installAttemptTotal = writable<number>(0);
/** 正在尝试的通道；与「已生效」的 installChannel 分开维护。 */
export const installAttemptChannel = writable<ChannelView | null>(null);
/**
 * 本次尝试的开始时间（毫秒时间戳），用于界面显示「已 X 秒」。
 *
 * 这里只存时间戳、**不引定时器**：秒数由「运行环境」页的每秒心跳派生（见 EnvironmentPage.svelte）。
 * 定时器放状态机里就得在 `finally` / `resetProgress` / `cancelPendingInstall` 三处都记得清，
 * 少一处就是泄漏。
 */
export const installAttemptStartedAt = writable<number>(0);
/** 进度百分比（服务端未给长度时为 0）。 */
export const installPercent = writable<number>(0);
/** 失败原因（原始错误；渲染时按当前语言出文案）。 */
export const installError = writable<unknown>(null);
/** 后端给出的降级/回退说明（结构化）。 */
export const installNote = writable<UserMessage | null>(null);
/** 待用户确认的「未经官方校验」中间态（tasks 4.5）。 */
export const pendingInstall = writable<InstallOutcome | null>(null);
/** 最近一次成功安装的结果，用于展示通道与校验结论。 */
export const lastInstall = writable<InstallOutcome | null>(null);

function formatBytes(bytes: number): string {
  const units = ['B', 'KB', 'MB', 'GB'];
  let value = bytes;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i += 1;
  }
  return i === 0 ? `${Math.round(value)} ${units[i]}` : `${value.toFixed(1)} ${units[i]}`;
}

/** 进度文案：由阶段 + 结构化字段派生，语言切换时自动重译。 */
export const installText = derived(
  [
    installAwaitingConfirmation,
    installPhase,
    installAsset,
    installDone,
    installTotal,
    installVersion,
    installReplaceStoppingAdb,
    installAttempt,
    t,
  ],
  ([
    $awaiting,
    $phase,
    $asset,
    $done,
    $total,
    $version,
    $stoppingAdb,
    $attempt,
    $t,
  ]): string => {
    if ($awaiting) return $t('install.phase.needsConfirmation');
    if (!$phase) return '';
    // 尝试事件不带资产：这一行由 DownloadProgress 的通道行负责。
    if ($attempt > 0 && !$asset) return '';
    const asset = $asset ?? '';
    switch ($phase) {
      case 'download':
        if (!asset) return '';
        if ($done === null) return $t('install.phase.preparing', { asset });
        if ($total !== null && $total > 0) {
          return $t('install.phase.downloadingTotal', {
            asset,
            done: formatBytes($done),
            total: formatBytes($total),
          });
        }
        return $t('install.phase.downloading', { asset, done: formatBytes($done) });
      case 'verify':
        return $t('install.phase.verifying', { asset });
      case 'extract':
        if ($done === null) return $t('install.phase.extracting', { asset });
        if ($total !== null && $total > 0) {
          return $t('install.phase.extractingTotal', { asset, done: $done, total: $total });
        }
        return $t('install.phase.extractingDone', { asset, done: $done });
      case 'replace':
        return $stoppingAdb
          ? $t('install.phase.replacingWithAdb')
          : $t('install.phase.replacing');
      case 'done':
        return $version
          ? $t('install.phase.done', { version: $version })
          : $t('install.phase.replacing');
      default:
        return '';
    }
  },
);

/**
 * 环境从「未就绪」变为「就绪」时清掉上一次的失败原因。
 *
 * 为什么需要：下载全部通道失败后，用户按提示手动把二进制放进约定目录、再点「重新检测」，
 * 环境已就绪但 `installError` 还挂着 —— 「运行环境」页的运行环境卡片会同时显示「已就绪」与一条
 * 红色的旧失败原因，自相矛盾且无法清掉。
 *
 * 为什么只在**跃迁**上清：写成「只要就绪就清」的话，「已有可用环境时重新下载失败」的提示
 * 会被紧随其后的任何一次刷新立刻抹掉 —— 那正是用户最需要看到的原因。
 */
let envWasReady = false;
envStatus.subscribe((s) => {
  const ready = s?.ready ?? false;
  if (ready && !envWasReady) installError.set(null);
  envWasReady = ready;
});

let listenerReady: Promise<void> | null = null;

/** 只注册一次进度事件监听（事件由后端 `emit`，见 `download::PROGRESS_EVENT`）。 */
function ensureListener(): Promise<void> {
  if (!listenerReady) {
    listenerReady = listen<InstallProgress>('scrcpy-install-progress', (event) => {
      const p = event.payload;
      installPhase.set(p.phase);
      installPercent.set(p.percent);
      installAsset.set(p.asset);
      installDone.set(p.done);
      installTotal.set(p.total);
      installVersion.set(p.version);
      installReplaceStoppingAdb.set(p.replaceStoppingAdb);
      if (p.channelView) installChannel.set(p.channelView);
      // 尝试信息是「粘性」的：只有带尝试信息的事件才更新，且一旦确定就保留到下次发起获取。
      // 否则紧随其后的下载进度事件（attempt = 0）会把「正在尝试第几条通道」抹掉。
      if (p.attempt > 0) {
        // 「已生效通道」必须与新一次尝试一起失效：它是粘性的，而通道行的取值里它**优先**于
        // 尝试信息 —— 不清的话，用户点「切换通道」或应用自动降级后，这一行会一直停在
        // 上一条通道的「下载通道：X」，直到新通道返回第一块数据才更新（用户据此以为切换没生效）。
        installChannel.set(null);
        installAttempt.set(p.attempt);
        installAttemptTotal.set(p.attemptTotal);
        installAttemptChannel.set(p.attemptChannelView);
        installAttemptStartedAt.set(Date.now());
      }
    }).then(() => undefined);
  }
  return listenerReady;
}

function resetProgress(): void {
  installPhase.set('');
  installAwaitingConfirmation.set(false);
  installAsset.set(null);
  installDone.set(null);
  installTotal.set(null);
  installVersion.set(null);
  installReplaceStoppingAdb.set(null);
  installPercent.set(0);
  installChannel.set(null);
  installAttempt.set(0);
  installAttemptTotal.set(0);
  installAttemptChannel.set(null);
  installAttemptStartedAt.set(0);
  installError.set(null);
  installNote.set(null);
}

/** 失败时只收起进度，保留失败原因与上一次安装结论。 */
function clearProgressFields(): void {
  installPhase.set('');
  installAwaitingConfirmation.set(false);
  installAsset.set(null);
  installDone.set(null);
  installTotal.set(null);
  installVersion.set(null);
  installReplaceStoppingAdb.set(null);
  installPercent.set(0);
  installChannel.set(null);
  installAttempt.set(0);
  installAttemptTotal.set(0);
  installAttemptChannel.set(null);
  installAttemptStartedAt.set(0);
}

/**
 * 发起一次获取：`bestFit` 用于首启缺失引导与「下载最佳适配版本」（构建期锁定的受支持版本），
 * `latest` 用于「下载最新版本」（官方最新发布）。
 */
export async function runInstall(mode: 'bestFit' | 'latest'): Promise<void> {
  await ensureListener();
  resetProgress();
  installRunning.set(true);
  try {
    const outcome = await startScrcpyInstall(mode);
    if (outcome.status === 'canceled') {
      // 取消不是错误：收起进度块、不写 installError、也不置 lastInstall（否则会留下
      // 一条「下载通道：…」的结论句，像是刚下完）。本次收尾由这里统一完成。
      resetProgress();
      return;
    }
    installChannel.set(outcome.channelView);
    installNote.set(outcome.noteMessage);
    if (outcome.status === 'needsConfirmation') {
      // 未经校验 → 必须由用户确认后才继续（tasks 4.5）。
      pendingInstall.set(outcome);
      installAwaitingConfirmation.set(true);
      return;
    }
    lastInstall.set(outcome);
    installPhase.set('done');
    installVersion.set(outcome.version);
    installPercent.set(100);
    await refreshEnv();
  } catch (e) {
    // 失败时清掉进度文案，只留失败原因（进度条随之收起）。
    clearProgressFields();
    installError.set(e);
  } finally {
    installRunning.set(false);
  }
}

/** 用户确认「未经官方校验仍继续」：复用已下载文件继续解压与替换。 */
export async function confirmPendingInstall(): Promise<void> {
  const pending = pendingOf();
  pendingInstall.set(null);
  installAwaitingConfirmation.set(false);
  if (!pending) return;
  installRunning.set(true);
  try {
    const outcome = await confirmScrcpyInstall();
    lastInstall.set(outcome);
    installNote.set(outcome.noteMessage);
    installPhase.set('done');
    installVersion.set(outcome.version);
    installPercent.set(100);
    await refreshEnv();
  } catch (e) {
    clearProgressFields();
    installError.set(e);
  } finally {
    installRunning.set(false);
  }
}

/**
 * 「切换通道」：中断当前通道的尝试，改用下一条可用通道重新下载（下载阶段可用）。
 *
 * 只发命令、不改状态：后端换通道后会重发「正在尝试通道 N/M」事件（进度随之归零），
 * 前端的状态由那串事件驱动。这里唯一要处理的是命令本身的失败 —— 它只在
 * 「当前没有进行中的下载」时发生，说明界面状态已过期，提示比静默无反应好排查。
 */
export async function switchChannel(): Promise<void> {
  try {
    await switchScrcpyChannel();
  } catch (e) {
    installError.set(e);
  }
}

/**
 * 「取消」：停止这次获取（中断在途下载/校验，并丢弃待确认的中间态）。
 *
 * 同样只发命令、不改状态：在途的 `runInstall` 会以 `status === 'canceled'` 正常返回，
 * 由它统一收尾。前端先清状态会让按钮的可用性与后端是否还在跑脱节。
 */
export async function cancelInstall(): Promise<void> {
  try {
    await cancelScrcpyInstall();
  } catch (e) {
    installError.set(e);
  }
}

/** 用户放弃未校验的安装：后端清掉临时文件，现有运行环境零改动。 */
export async function cancelPendingInstall(): Promise<void> {
  pendingInstall.set(null);
  installAwaitingConfirmation.set(false);
  resetProgress();
  await cancelInstall();
}

function pendingOf(): InstallOutcome | null {
  let value: InstallOutcome | null = null;
  pendingInstall.subscribe((v) => (value = v))();
  return value;
}
