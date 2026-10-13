<script lang="ts">
  import { page, collapsed, aboutOpen } from '../lib/store';
  import {
    TabletSmartphone,
    Monitor,
    Video,
    Volume2,
    Gamepad2,
    Disc,
    AppWindow,
    LayoutGrid,
    ScrollText,
    Settings,
    Info,
    ChevronsLeft,
  } from 'lucide-svelte';
  import Brand from './Brand.svelte';
  import { tip } from '../lib/tooltip';

  // 11 个平级导航入口：设备 + 8 个默认参数页 + 日志页 + 「关于」。
  // 页键即「页面身份」，与入口名一致（`advanced` = 高级、`environment` = 运行环境）；
  // 注意别与默认参数的**分组键**（同样是 `apps` / `advanced`，但属落盘配置字段）混为一谈。
  // 「日志」插在「高级」与「运行环境」之间；「运行环境」是末位**功能**入口，
  // 「关于」（`dialog: true`）是最后一个入口 —— **入口数 11 与页键数 10 不是同一个数**：
  // 「关于」不是页面，点它只开弹窗，不切页、不动选中态与标题（app-shell 规格「统一导航入口」）。
  // 「日志」的图标与设备页跳往该页的「日志」按钮同源（ScrollText）—— 同一概念不引入两个图标。
  type NavItem = { key: string; label: string; icon: typeof Monitor; dialog?: true };

  const items: Array<NavItem | { sep: true }> = [
    { key: 'devices', label: '设备', icon: TabletSmartphone },
    { key: 'source', label: '显示源', icon: Monitor },
    { key: 'video', label: '视频', icon: Video },
    { key: 'audio', label: '音频', icon: Volume2 },
    { key: 'control', label: '控制', icon: Gamepad2 },
    { key: 'record', label: '录制', icon: Disc },
    { key: 'window', label: '窗口', icon: AppWindow },
    { key: 'advanced', label: '高级', icon: LayoutGrid },
    { sep: true },
    { key: 'diagnostics', label: '日志', icon: ScrollText },
    { key: 'environment', label: '运行环境', icon: Settings },
    // 末位：不是页面，只开「关于」弹窗（`aboutOpen` 是它与品牌行共用的开关）
    { key: 'about', label: '关于', icon: Info, dialog: true },
  ];

  function select(it: NavItem) {
    if (it.dialog) {
      aboutOpen.set(true);
      return;
    }
    page.set(it.key);
  }
  function toggle() {
    collapsed.update((v) => !v);
  }
</script>

<aside class="sidebar">
  <Brand />
  <nav class="nav">
    {#each items as it}
      {#if 'sep' in it}
        <div class="nav-sep"></div>
      {:else}
        <!-- 收起态下只剩图标，hover（或键盘聚焦）时以自绘提示显示该入口的标题（规格 `hover-tooltip`）；
             取值复用与下方 nav-text 同一个 label，不另立一份标题表。
             展开态传空即不挂这条提示：文字本已可见，再来一条只是重复。
             提示落到栏外（`placement: 'right'`）—— 侧边栏自身会横向裁剪，提示层挂在应用级才不被裁。 -->
        <div
          class="nav-item"
          class:active={!it.dialog && $page === it.key}
          use:tip={$collapsed ? { text: it.label, placement: 'right' } : null}
          on:click={() => select(it)}
          on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && select(it)}
          role="button"
          tabindex="0"
        >
          <svelte:component this={it.icon} />
          <span class="nav-text">{it.label}</span>
        </div>
      {/if}
    {/each}
  </nav>
  <button class="collapse-btn" on:click={toggle}>
    <ChevronsLeft />
    <span class="nav-text">收起</span>
  </button>
</aside>
