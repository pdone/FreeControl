<script lang="ts">
  // 首启缺失引导（spec `scrcpy-environment`「首次启动按需获取」）。
  //
  // 引导**仅作展示**：说明缺什么、约定目录在哪、可自动获取最佳适配版本；
  // 只提供「稍后再说」与「下载并安装」两项操作 —— 进度、失败原因、打开目录、
  // 重新检测都由「运行环境」页承载，本弹窗不再就地下载（获取流程全仓只剩一套实现，
  // 即「运行环境」页的运行环境卡片；进度块也只在那一份）。
  //
  // 「下载并安装」是一次动作的三步（design D2 / D3）：先关引导（否则弹窗会盖住
  // 它刚跳过去的那一页）→ 切到「运行环境」页 → 就地发起最佳适配版本的获取。
  // 触发条件只有一个：后端判定运行环境**未就绪**且本会话内未被用户处理过；
  // 环境就绪后引导自动不再展示，「稍后再说」只在本会话内抑制（不持久化，
  // 下次启动若仍缺失会再提示）。
  import { envStatus, scrcpyDir, envGuideDismissed, page } from '../lib/store';
  import { installRunning, runInstall } from '../lib/install';
  import { AlertTriangle, Download } from 'lucide-svelte';

  $: show = !!$envStatus && !$envStatus.ready && !$envGuideDismissed;
  $: missingText = $envStatus?.missing.join('、') ?? '';

  function startAcquire(): void {
    // 顺序不可换：只跳不关会让弹窗盖住目标页（design D2）。
    envGuideDismissed.set(true);
    page.set('environment');
    // 首启取最佳适配版本而非最新（design D3）：不依赖官方发布查询，最确定。
    // runInstall 自带收尾（失败写 installError、由「运行环境」页展示），无需在此等待。
    runInstall('bestFit');
  }
</script>

{#if show}
  <div class="overlay show">
    <div class="modal">
      <div class="ico warn"><AlertTriangle /></div>
      <h2>需要 scrcpy 运行环境</h2>
      <p>
        未检测到可用的 scrcpy 二进制，因此现在还不能发起投屏会话。
        点击“下载并安装”按钮可一键部署scrcpy环境。
      </p>
      <div class="field">
        <span class="flabel" style="font-weight: 800;">scrcpy 目录</span>
        <div class="env-path">{$scrcpyDir}</div>
        <div class="hint">
          缺少：<b>{missingText}</b> —— 下载并安装会把 scrcpy 与 adb 一并解压到该目录。
        </div>
      </div>
      <div class="actions">
        <button class="btn ghost" on:click={() => envGuideDismissed.set(true)}>稍后再说</button>
        <button class="btn primary" on:click={startAcquire} disabled={$installRunning}>
          <Download /> 下载并安装
        </button>
      </div>
    </div>
  </div>
{/if}
