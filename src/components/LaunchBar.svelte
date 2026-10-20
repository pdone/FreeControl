<script lang="ts">
  // 命令条（design D11）：承担「预览 + 复制 + 会话入口」。会话入口是本变更新增的第二个入口
  // （原先只设「预览 + 复制」，发起会话唯一入口是设备卡片上的「连接 / 断开」；该约定已按本次变更推翻）。
  // 它只对**选中的**那台设备生效，并随该设备状态在「连接 / 断开」间切换，与设备卡片共用同一套 store 逻辑。
  //
  // 预览的取值来源与将要执行的命令**同一份**：`effectiveParams` = 默认参数叠加
  // 该设备**启用状态**的覆盖（store 里已经按 design D5 的规则合并好）。
  // 校验失败的字段不参与拼装（tasks 7.3），并在右侧给出提示 —— 界面看到的
  // 就是会执行的，非法值不会悄悄溜进命令里。
  import { afterUpdate, onMount } from 'svelte';
  import { Terminal, Copy, Check, Zap, ZapOff, AlertTriangle } from 'lucide-svelte';
  import {
    devices,
    selectedDeviceId,
    effectiveParams,
    effectiveErrors,
    deviceStatus,
    connectDevice,
    disconnectDevice,
    canConnect,
    connectBlockedReason,
  } from '../lib/store';
  import { buildCommandParts, commandText } from '../lib/command';
  import { copyText } from '../lib/clipboard';
  import { errText } from '../lib/install';
  import { tip } from '../lib/tooltip';
  import type { DeviceStatusView } from '../lib/types';

  /** 选中设备的运行期状态；未选中或尚未探测出结果时按「未知」处理（与设备页一致）。 */
  let connStatus: DeviceStatusView = 'unknown';

  /** 命令条上的会话入口：在途标志（连接与断开共用一个）。 */
  let busy = false;
  /** 会话操作失败的信息（短语 + 详情）—— 命令条自持：`sessionNotice` 只在设备页渲染，停在其他页面时看不到。 */
  let actionFail: { label: string; detail: string } | null = null;

  // 只有「选中 + 连接标识已知」时才展示命令：标识待检测时命令是不完整的，
  // 与其展示一条残缺命令，不如继续给占位提示（tasks 8.3）。
  $: dev = $devices.find((d) => d.id === $selectedDeviceId) ?? null;
  $: parts =
    dev && dev.connectionId
      ? buildCommandParts(dev.connectionId, $effectiveParams, $effectiveErrors)
      : null;
  $: errors = Object.values($effectiveErrors);

  // 命令预览的截断判定（design D6）：单行 `nowrap + ellipsis` 真的省略了内容时，
  // 才把完整命令挂到 hover 提示上；未截断时传 null，不重复可见文字。
  // `afterUpdate` 覆盖内容变化，`ResizeObserver` 覆盖侧栏收起/展开与窗口缩放带来的宽度变化。
  let previewEl: HTMLElement | null = null;
  let previewTruncated = false;
  let previewObserver: ResizeObserver | null = null;

  function measurePreview(): void {
    const next = !!previewEl && previewEl.scrollWidth > previewEl.clientWidth + 1;
    if (next !== previewTruncated) previewTruncated = next;
  }

  afterUpdate(measurePreview);
  onMount(() => {
    previewObserver = new ResizeObserver(measurePreview);
    return () => {
      previewObserver?.disconnect();
      previewObserver = null;
    };
  });
  $: if (previewObserver) {
    previewObserver.disconnect();
    if (previewEl) previewObserver.observe(previewEl);
  }

  // 会话入口的形态随选中设备状态切换（design D2）：
  // 已连接 → 「断开」态（结束会话）；未连接且可连接 → 「连接」态（发起会话）；
  // 其余（未选中 / 未连接且不可连接）置灰。判定复用 store，命令条不自算一套。
  $: connStatus = (dev ? $deviceStatus[dev.id] : undefined) ?? 'unknown';
  $: isConnected = !!dev && connStatus === 'connected';
  $: isConnectable = !!dev && canConnect(dev, connStatus);
  // 可点：未在途，且处于「断开」态或可连接的「连接」态（与设备卡片一样「没有可做的事就置灰」）。
  $: entryEnabled = !!dev && !busy && (isConnected || isConnectable);
  $: entryLabel = dev ? `${isConnected ? '断开' : '连接'}设备 ${dev.alias}` : '会话入口';
  $: entryTitle = !dev
    ? '请先选择一台设备'
    : busy
      ? '正在处理…'
      : isConnected
        ? '断开该设备的会话'
        : isConnectable
          ? '连接该设备'
          : connectBlockedReason(dev, connStatus) || '当前无法连接该设备';

  /** 复制成功反馈的显示时长（与原型一致：按钮短暂变为对勾）。 */
  const COPIED_MS = 1400;
  let copied = false;
  let copyTimer: ReturnType<typeof setTimeout> | null = null;
  let copyErr = '';

  /**
   * 一键复制（tasks 8.4）。
   *
   * 复制内容是 `commandText(parts)` —— 由命令三段（可执行文件 / `-s` 标识 / 参数数组）
   * 直接拼成的字符串，**不经过任何 DOM 序列化**，因此界面上的 `<b>` 高亮与 `.dev`
   * 着色不可能混进剪贴板。
   *
   * 没有命令可复制时（未选设备 / 连接标识未知）直接返回：不写剪贴板、也不给成功反馈
   * （tasks 8.3）。复制失败时同样不给成功反馈，而是把原因显示出来。
   */
  async function copyCommand(): Promise<void> {
    const p = parts;
    if (!p) return;
    copyErr = '';
    try {
      await copyText(commandText(p));
    } catch (e) {
      copied = false;
      copyErr = e instanceof Error ? e.message : String(e);
      return;
    }
    copied = true;
    if (copyTimer) clearTimeout(copyTimer);
    copyTimer = setTimeout(() => {
      copied = false;
      copyTimer = null;
    }, COPIED_MS);
  }

  /**
   * 命令条上的会话入口（与设备卡片的「连接 / 断开」同一套 store 逻辑）：
   * 已连接的设备执行断开，未连接且可连接的执行连接；其余一律不提交。
   *
   * 失败写进命令条的局部信息（渲染为 `.lb-warn`）—— `sessionNotice` 只在设备页渲染，
   * 停在参数页时看不到它，故失败反馈由命令条自身承载。成功不额外提示：设备状态随即翻转，
   * 入口的形态（连接 ↔ 断开）也跟着翻，这一步本身就是成功信号。
   */
  async function toggleSession(): Promise<void> {
    const d = dev;
    if (!d || busy) return;
    const disconnect = isConnected;
    if (!disconnect && !isConnectable) return;
    busy = true;
    actionFail = null;
    try {
      await (disconnect ? disconnectDevice(d.id) : connectDevice(d.id));
    } catch (e) {
      actionFail = { label: disconnect ? '断开失败' : '连接失败', detail: errText(e) };
    } finally {
      busy = false;
    }
  }
</script>

<div class="launchbar">
  <!-- 标签仅保留图标（config.yaml ④：控件标签的存废）：图标是纯装饰，命令本身就在右侧预览里，
       故标记 aria-hidden，不给它可访问名称、也不给提示。 -->
  <span class="lb-label" aria-hidden="true"><Terminal /></span>
  {#if parts}
    <!-- 参数之间必须显式写空格：Svelte 的 each 块不会自动插入空白节点，
         直接相邻的 <b> 会连成 `...h265-b 16M` 这样不可读的命令。 -->
    <div
      class="cmd-preview"
      bind:this={previewEl}
      use:tip={previewTruncated && parts ? commandText(parts) : null}
    >
      {parts.bin} -s <span class="dev">{parts.serial}</span>
      {#each parts.args as a}<b>{a}</b>{' '}{/each}
    </div>
  {:else if dev}
    <div class="cmd-preview">
      该设备的连接标识尚未确定（USB serial 待检测），暂无法拼装命令
    </div>
  {:else}
    <div class="cmd-preview">选择一台设备查看命令</div>
  {/if}
  {#if errors.length > 0}
    <span class="lb-warn" use:tip={errors.join('\n')}>
      <AlertTriangle /> {errors.length} 项参数非法，已不参与拼装
    </span>
  {/if}
  {#if copyErr}
    <span class="lb-warn" use:tip={copyErr}><AlertTriangle /> 复制失败</span>
  {/if}
  {#if actionFail}
    <span class="lb-warn" use:tip={actionFail.detail}><AlertTriangle /> {actionFail.label}</span>
  {/if}
  <!-- 两枚按钮都会条件禁用，而 `disabled` 元素不派发指针事件：故提示的接线挂在外层承载元素上
       （见 app.css 的 `.tip-host`），由它代表内部那枚按钮。 -->
  <span
    class="tip-host"
    use:tip={!parts
      ? '尚未生成命令，无可复制内容'
      : copied
        ? '已复制到剪贴板'
        : '复制命令（纯文本）'}
  >
    <button
      class="copy-btn"
      class:ok={copied}
      disabled={!parts}
      aria-label={copied ? '已复制命令' : '复制命令'}
      on:click={copyCommand}
    >
      {#if copied}<Check />{:else}<Copy />{/if}
    </button>
  </span>
  <!-- 会话入口：仅图标，排在复制按钮之后；随选中设备状态在「连接 / 断开」间切换（见 entryTitle 的判定）。 -->
  <span class="tip-host" use:tip={entryTitle}>
    <button
      class="connect-btn"
      disabled={!entryEnabled}
      aria-label={entryLabel}
      on:click={toggleSession}
    >
      {#if isConnected}<ZapOff />{:else}<Zap />{/if}
    </button>
  </span>
</div>
