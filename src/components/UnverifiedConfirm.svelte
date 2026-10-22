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
  import { channelLabel } from '../lib/channel';
  import { t } from '../lib/i18n';
  import { errorTextOf } from '../lib/message';
  import { AlertTriangle, Download } from 'lucide-svelte';
  import { tip } from '../lib/tooltip';
</script>

{#if $pendingInstall}
  <div class="overlay show">
    <div class="modal">
      <div class="ico warn"><AlertTriangle /></div>
      <h2>{$t('unverified.title')}</h2>
      <p>
        {errorTextOf($pendingInstall.unverifiedReasonMessage, $t)}{$t('unverified.bodyAfter')}
      </p>
      <div class="field">
        <span class="flabel">{$t('unverified.source')}</span>
        <div class="env-path" use:tip={$pendingInstall.channelUrl}
          >{channelLabel($pendingInstall.channelView, $t)}</div
        >
        <div class="hint">
          {$t('unverified.assetBefore')}<code>{$pendingInstall.asset}</code>{$t(
            'unverified.assetMid',
          )}<code>{$scrcpyDir}</code>{$t('unverified.assetAfter')}
        </div>
      </div>
      {#if $pendingInstall.noteMessage}
        <p>{errorTextOf($pendingInstall.noteMessage, $t)}</p>
      {/if}
      <div class="actions">
        <button class="btn ghost" on:click={cancelPendingInstall} disabled={$installRunning}>
          {$t('unverified.cancel')}
        </button>
        <button class="btn primary" on:click={confirmPendingInstall} disabled={$installRunning}>
          <Download /> {$t('unverified.confirm')}
        </button>
      </div>
    </div>
  </div>
{/if}
