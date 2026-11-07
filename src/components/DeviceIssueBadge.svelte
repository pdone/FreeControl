<script lang="ts">
  // 设备行内的问题指示：详细模式是「图标 + 短标签」，简洁模式只有图标。
  // 完整原因只进入提示层与可访问名称，不参与行布局 —— 长 adb 原文因此不会撑高卡片。
  import { AlertTriangle } from 'lucide-svelte';
  import { tip } from '../lib/tooltip';

  export let label: string;
  export let detail: string;
  export let tone: 'danger' | 'warn' = 'danger';
  export let iconOnly = false;
</script>

<button
  type="button"
  class="dev-issue {tone}"
  class:icon-only={iconOnly}
  tabindex="0"
  aria-label={`${label}${detail ? `：${detail}` : ''}`}
  use:tip={detail}
  on:click|stopPropagation={() => {}}
>
  <AlertTriangle />
  {#if !iconOnly}<span class="dev-issue-label">{label}</span>{/if}
</button>