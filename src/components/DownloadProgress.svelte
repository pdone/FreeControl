<script lang="ts">
  // 下载进度块：运行环境卡的下载进度与通道反馈。获取流程如今只发生在「运行环境」页，
  // 故本组件全仓只有一份引用 —— 不再与首启缺失引导共用，因为引导不再就地下载。
  //
  // 历史教训：这段标记曾经与首启引导各写一份（连 `channelLine` 都抄了两遍），结果是
  // 改了一处、漏了另一处（用户按截图反馈）。所以即便现在只剩一个使用方，也仍以
  // 「单一组件 + 共享 store」的形式存在 —— 一旦未来哪里又要就地下载，复用成本
  // 应当接近零。
  //
  // 只负责渲染：进度、通道、尝试信息与最终结论全部来自 `install.ts` 的 store，本组件不持有状态。
  import { onDestroy, onMount } from 'svelte';
  import { Shuffle, X } from 'lucide-svelte';
  import { proxies } from '../lib/store';
  import { tip } from '../lib/tooltip';
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
   * 通道与校验结论（tasks 4.2 / 4.4 / 4.5）：下载中标注当前所用通道，完成后补一句校验结论，
   * 让「走了哪条通道、是否校验过」始终可见。
   *
   * 传输中**也**展示通道（用户先要求去掉、随后要求恢复，现定稿在"展示"）。代价是本行内容超宽：
   * 实测 438px 可用宽下，进度文案自然 311px 只显到 275px、通道「GitHub 直连」128px 只显到 114px，
   * 被截掉的全文都在各自的 `title` 里。若哪天要「零省略」，只能再从文案本身上下刀
   * （去掉资产名或体积数字）或缩短通道标签 —— **别把某一格改回可换行**，那会让整行变成两行。
   *
   * 三档的先后顺序（已生效通道优先于尝试信息）**只在同一次尝试内成立**：新一次尝试开始
   * （用户点「切换通道」，或某条通道失败/停滞后的自动降级）时由 `install.ts` 清空
   * `$installChannel`，这一行随即回到尝试行「正在尝试通道 N/M：<新通道>（已 0 秒）」——
   * 不等新通道返回数据，也不残留上一条通道。改动这里的判定顺序前先读 `install.ts` 的监听。
   *
   * 「尚未有尝试事件」那一档要写成中性的「正在准备下载…」，**不能**写成
   * 「正在尝试第 1 条通道」—— 那会让「准备中」与「正在试第一条」长得一模一样，
   * 而这正是最初「这句话长时间不动」的根源。
   */
  $: channelLine = (() => {
    if ($installError) return '';
    if ($installRunning) {
      if ($installChannel) return `下载通道：${$installChannel}`;
      if ($installAttempt > 0) {
        return (
          `正在尝试通道 ${$installAttempt}/${$installAttemptTotal}：` +
          `${$installAttemptChannel}（已 ${elapsedSec} 秒）`
        );
      }
      return '正在准备下载…';
    }
    if ($lastInstall) {
      // 结论句刻意短：整行可用宽只有 ~438px，而通道标签本身就占 128~228px（代理标签含完整 URL）。
      // 实测旧句「…校验通过，已原子替换运行环境目录」共 426px，会把同一行的安装结果文案挤掉一截。
      // 「安装成功 / 已替换目录」由左侧文案（`已安装 scrcpy vX`）表达，本行只留「校验与否」
      // 这一条别处没有的信息；全文仍在 title 里。
      const verdict = $lastInstall.verified ? 'SHA256SUMS 校验通过' : '未经官方校验';
      return `下载通道：${$lastInstall.channel} · ${verdict}。`;
    }
    return $installNote;
  })();

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
  $: canCancel =
    $installRunning && ($installPhase === 'download' || $installPhase === 'verify');
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
          <button
            class="btn ghost sm"
            on:click={switchChannel}
            use:tip={'中断当前通道的下载，改用其他通道重新下载（当前进度会作废）'}
          >
            <Shuffle /> 切换通道
          </button>
        {/if}
        {#if canCancel}
          <button
            class="btn ghost sm"
            on:click={cancelInstall}
            use:tip={'停止这次获取：清掉临时文件，现有运行环境保持不变'}
          >
            <X /> 取消
          </button>
        {/if}
      </div>
    {/if}
  </div>
{/if}
