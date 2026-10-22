<script lang="ts">
  // 下载进度块：运行环境卡的下载进度与通道反馈。获取流程如今只发生在「运行环境」页，
  // 故本组件全仓只有一份引用 —— 不再与首启缺失引导共用，因为引导不再就地下载。
  //
  // 进度文案、通道标签与结论句全部由结构化 store + 当前语言派生；本组件不持有进度状态。
  import { onDestroy, onMount } from 'svelte';
  import { Shuffle, X } from 'lucide-svelte';
  import { proxies } from '../lib/store';
  import { channelLabel } from '../lib/channel';
  import { t, type MessageKey, type MessageParams } from '../lib/i18n';
  import { errorTextOf } from '../lib/message';
  import { tip } from '../lib/tooltip';
  import type { ChannelView, InstallOutcome, UserMessage } from '../lib/types';
  import {
    installRunning,
    installPhase,
    installText,
    installPercent,
    installChannel,
    installAttempt,
    installAttemptTotal,
    installAttemptChannel,
    installAttemptStartedAt,
    installError,
    installNote,
    lastInstall,
    switchChannel,
    cancelInstall,
  } from '../lib/install';

  type Translate = (key: MessageKey, params?: MessageParams) => string;

  // 每秒心跳：只为让「已 X 秒」跟着走（`install.ts` 刻意不带定时器 —— 放状态机里就得在
  // `finally` / `resetProgress` / `cancelPendingInstall` 三处都记得清，少一处就是泄漏）。
  let now = Date.now();
  let tickTimer: ReturnType<typeof setInterval> | null = null;
  onMount(() => {
    tickTimer = setInterval(() => (now = Date.now()), 1000);
  });
  onDestroy(() => {
    if (tickTimer) clearInterval(tickTimer);
  });

  /**
   * 本次尝试已耗时的秒数。
   *
   * `now` 必须**直接出现在这个 `$:` 表达式里** —— Svelte 4 只跟踪表达式中直接出现的变量，
   * 把计算挪进函数体就会静默失去更新（本仓已知坑）。
   */
  $: elapsedSec =
    $installRunning && $installAttemptStartedAt
      ? Math.max(0, Math.floor((now - $installAttemptStartedAt) / 1000))
      : 0;

  /**
   * 通道与校验结论（tasks 4.2 / 4.4 / 4.5）：下载中标注当前所用通道，完成后补一句校验结论。
   * 三档顺序：已生效通道 → 正在尝试 → 准备中；完成后显示结论或回退说明。
   */
  function channelLineText(
    error: unknown,
    running: boolean,
    channel: ChannelView | null,
    attempt: number,
    attemptTotal: number,
    attemptChannel: ChannelView | null,
    elapsed: number,
    last: InstallOutcome | null,
    note: UserMessage | null,
    tr: Translate,
  ): string {
    if (error) return '';
    if (running) {
      if (channel) return tr('install.channelLine', { channel: channelLabel(channel, tr) });
      if (attempt > 0) {
        return tr('install.attempt', {
          index: attempt,
          total: attemptTotal,
          channel: channelLabel(attemptChannel, tr),
          seconds: elapsed,
        });
      }
      return tr('install.preparing');
    }
    if (last) {
      const verdict = last.verified
        ? tr('install.verdict.verified')
        : tr('install.verdict.unverified');
      return tr('install.channelLineVerdict', {
        channel: channelLabel(last.channelView, tr),
        verdict,
      });
    }
    return note ? errorTextOf(note, tr) : '';
  }

  // `$t` 作为显式参数出现在顶层 reactive 表达式里，语言切换时这一行会重算。
  $: channelLine = channelLineText(
    $installError,
    $installRunning,
    $installChannel,
    $installAttempt,
    $installAttemptTotal,
    $installAttemptChannel,
    elapsedSec,
    $lastInstall,
    $installNote,
    $t,
  );

  /**
   * 候选通道数 = GitHub 直连 + 配置里的代理地址。
   *
   * 代理被删光时只剩直连一条，「切换通道」无处可去 —— 按钮据此禁用，而不是给出一个
   * 必然失败的交互（spec「下载过程的人工干预」：切换要能改用**其他**通道）。
   */
  $: channelCount = $proxies.proxies.length + 1;

  /**
   * 两项操作的可用阶段（spec「下载过程的人工干预」）：
   * - 切换只在**下载资产**阶段：校验阶段资产已下完，换通道只会把它重下一遍，故不提供；
   * - 取消覆盖下载与取校验值两个阶段。
   * 进入解压 / 替换后网络已结束（替换是原子的），两者都不再出现。
   */
  $: canSwitch = $installRunning && $installPhase === 'download' && channelCount > 1;
  $: canCancel = $installRunning && ($installPhase === 'download' || $installPhase === 'verify');
</script>

{#if $installRunning || ($installText && !$installError) || channelLine}
  <div class="dl">
    <!-- 通道信息与进度文案同行展示（不另起一行）：通道那格吃掉剩余宽度，
         百分比恒钉最右；容器窄到放不下时在各自格内换行而不是溢出。 -->
    <div class="dl-head">
      <!-- 两个可变格都恒为单行、超出部分省略（见 app.css 的 `.dl-head` 规则）。被省略的那截
           不靠 title 看：这两格在可拖选白名单里（app.css 的 `user-select: text`），拖选即得全文。 -->
      {#if $installText}
        <span class="dl-text">{$installText}</span>
      {/if}
      {#if channelLine}
        <span class="dl-channel">{channelLine}</span>
      {/if}
      <span class="dl-percent">{$installPercent}%</span>
    </div>
    <div class="dl-bar"><i style="width:{$installPercent}%"></i></div>
    {#if canSwitch || canCancel}
      <div class="dl-actions">
        {#if canSwitch}
          <button class="btn ghost sm" on:click={switchChannel} use:tip={$t('install.switchTip')}>
            <Shuffle /> {$t('install.switch')}
          </button>
        {/if}
        {#if canCancel}
          <button class="btn ghost sm" on:click={cancelInstall} use:tip={$t('install.cancelTip')}>
            <X /> {$t('install.cancel')}
          </button>
        {/if}
      </div>
    {/if}
  </div>
{/if}
