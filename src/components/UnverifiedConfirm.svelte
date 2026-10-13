<script lang="ts">
  // 「无法获取官方校验值」时的二次确认（tasks 4.5 / spec「下载完整性校验」）。
  //
  // 触发条件：后端在下载完成后判定本次下载**未经官方校验**（SHA256SUMS 拉取失败，
  // 或其中缺少本次资产条目），此时不会继续解压写入，而是把结论放在 `pendingInstall`。
  // 只有用户在此确认后才继续；取消则清掉临时文件、现有运行环境零改动。
  //
  // 该弹窗挂在应用级（App.svelte），因为首启引导与「运行环境」页的运行环境卡片都可能触发它。
  import { scrcpyDir } from '../lib/store';
  import {
    pendingInstall,
    confirmPendingInstall,
    cancelPendingInstall,
    installRunning,
  } from '../lib/install';
  import { AlertTriangle, Download } from 'lucide-svelte';
  import { tip } from '../lib/tooltip';
</script>

{#if $pendingInstall}
  <div class="overlay show">
    <div class="modal">
      <div class="ico warn"><AlertTriangle /></div>
      <h2>本次下载未经官方校验</h2>
      <p>
        {$pendingInstall.unverifiedReason}。无法确认下载内容与官方发布是否完全一致，
        可能由网络拦截或官方发布文件变动导致。
      </p>
      <div class="field">
        <span class="flabel">下载来源</span>
        <div class="env-path" use:tip={$pendingInstall.channelUrl}>{$pendingInstall.channel}</div>
        <div class="hint">
          资产 <code>{$pendingInstall.asset}</code> 已下载完成但尚未校验。继续安装将解压并替换
          <code>{$scrcpyDir}</code> 中的全部文件。
        </div>
      </div>
      {#if $pendingInstall.note}
        <p>{$pendingInstall.note}</p>
      {/if}
      <div class="actions">
        <button class="btn ghost" on:click={cancelPendingInstall} disabled={$installRunning}>
          取消（不写入）
        </button>
        <button class="btn primary" on:click={confirmPendingInstall} disabled={$installRunning}>
          <Download /> 继续安装
        </button>
      </div>
    </div>
  </div>
{/if}
