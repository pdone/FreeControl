<script lang="ts">
  import {
    page,
    deviceCounts,
    compat,
    envStatus,
    windowDecorated,
    windowMaximized,
    minimizeWindow,
    toggleWindowMaximize,
    closeWindow,
  } from '../lib/store';
  import { AlertTriangle, Minus, Square, Copy, X } from 'lucide-svelte';
  import { tip } from '../lib/tooltip';

  // 页键 ↔ 标题（页键是「页面身份」，取自侧边栏入口名；与默认参数的**分组键**
  // `apps` / `advanced` 无关 —— 后者是落盘配置的字段名，不随页键改名）。
  const titles: Record<string, string> = {
    devices: '设备',
    source: '显示源',
    video: '视频',
    audio: '音频',
    control: '控制',
    record: '录制',
    window: '窗口',
    advanced: '高级',
    diagnostics: '日志',
    environment: '运行环境',
  };

  $: title = titles[$page] ?? '设备';
  // 三项计数只从 store 的 `deviceCounts` 取。这里是这三项计数在界面上的唯一展示位置 ——
  // 设备页不再重复同一份计数，也就不会有第二个位置算出不同数字。
  $: total = $deviceCounts.total;
  $: online = $deviceCounts.online;
  $: connected = $deviceCounts.connected;
  // 「环境未就绪」与「非最佳适配版本」是两回事（spec 两条独立要求）：
  // 前者是没装好（缺 scrcpy / adb），后者是版本不一致。二者都进黄色叹号，
  // 但文案必须分开 —— 首启缺环境时笼统报「非最佳适配版本」会把用户引向错误的处置方向。
  $: envReady = $envStatus?.ready ?? false;
  $: envOk = envReady && $compat?.status === 'ok';
  $: envWarnText = envReady ? '非最佳适配版本' : '环境未就绪';
  $: envWarnTitle = envReady
    ? '非最佳适配版本，点击前往「运行环境」页查看原因'
    : '运行环境未就绪（缺少 scrcpy / adb），点击前往「运行环境」页处理';

  // ── 两个指示的跳转（spec「顶部状态指示」/「版本兼容性指示」，design D7）──
  //
  // 两个指示在**任何状态**下点击都跳转：设备概况 → 「设备」页，运行环境指示 → 「运行环境」页。
  // 跳转与状态无关是刻意的 —— 同一个控件按状态给两种结果，用户学不到稳定预期。
  // 原先挂在环境指示上的「非最佳适配」原因说明条已迁到「运行环境」页
  // （见 `EnvironmentPage.svelte` 的运行环境卡：「查看原因」就地展开），顶栏不再承载说明条。
  function goDevices(): void {
    page.set('devices');
  }

  function goEnvironment(): void {
    page.set('environment');
  }

  /** 键盘激活：Enter / 空格与点击等价（两个指示都要对键盘用户可用）。 */
  function onPillKeydown(e: KeyboardEvent, go: () => void): void {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      go();
    }
  }
  // ── 自绘标题栏（app-shell 规格「自定义标题栏」）──
  //
  // Windows 上窗口不带系统标题栏（后端在 `setup` 里关掉装饰，见 `main.rs::disable_system_decorations`），
  // 这条顶栏因此兼作标题栏：空白处可拖拽、双击最大化 / 还原，右端三枚控件贴窗口右缘。
  //
  // `data-tauri-drag-region="deep"` 由运行环境的拖拽脚本解释：**子树内任意处可拖**，而可点击元素
  // （三枚控件与下面两个状态指示 —— 都是 button 或 role=button）自带阻断，不会被拖拽吞掉点击。
  // 双击最大化也由它承担（走 `internal-toggle-maximize`，该权限已在 `core:default` 里）。
  $: maxLabel = $windowMaximized ? '向下还原' : '最大化';
</script>

<!-- `data-tauri-drag-region="deep"`：仅当本窗口是自绘标题栏时才该拖 —— 不过该属性在带系统标题栏的
     窗口上也无害（那里顶部是系统的非客户区，脚本根本收不到命中），故无条件挂着，少一处平台分支。 -->
<div class="topbar" data-tauri-drag-region="deep">
  <div class="title">{title}</div>
  <div class="spacer"></div>
  <!-- 有在线设备（含已连接）时呈成功色：与运行环境指示共用 .pill.ok 一套配色，
       让顶栏一眼可见「有没有设备在线」，判定口径与计数同源（`$deviceCounts.online`）。 -->
  <span
    class="pill clickable"
    class:ok={online > 0}
    role="button"
    tabindex="0"
    use:tip={'回到设备页'}
    on:click={goDevices}
    on:keydown={(e) => onPillKeydown(e, goDevices)}
  >
    <span class="dot"></span>{total} 台设备 · {online} 在线 · {connected} 已连接
  </span>
  {#if !$envStatus}
    <span
      class="pill clickable"
      role="button"
      tabindex="0"
      use:tip={'前往运行环境页'}
      on:click={goEnvironment}
      on:keydown={(e) => onPillKeydown(e, goEnvironment)}
    >
      <span class="dot"></span>检测运行环境…
    </span>
  {:else if envOk}
    <span
      class="pill env ok clickable"
      role="button"
      tabindex="0"
      use:tip={'前往运行环境页'}
      on:click={goEnvironment}
      on:keydown={(e) => onPillKeydown(e, goEnvironment)}
    >
      <span class="dot"></span>scrcpy 就绪
    </span>
  {:else}
    <span
      class="pill env warn clickable"
      role="button"
      tabindex="0"
      use:tip={envWarnTitle}
      on:click={goEnvironment}
      on:keydown={(e) => onPillKeydown(e, goEnvironment)}
    >
      <AlertTriangle /> {envWarnText}
    </span>
  {/if}
  <!-- 三枚窗口控件：自左至右 最小化 / 最大化（向下还原）/ 关闭。
       `{#if}` 的判据是**窗口实际的装饰状态** —— 有系统标题栏（Linux）时整条不呈现，
       界面上不会出现两组窗口控件。 -->
  {#if !$windowDecorated}
    <div class="win-controls">
      <button
        class="win-btn"
        type="button"
        use:tip={'最小化'}
        aria-label="最小化"
        on:click={minimizeWindow}
      >
        <Minus />
      </button>
      <button
        class="win-btn"
        type="button"
        use:tip={maxLabel}
        aria-label={maxLabel}
        on:click={toggleWindowMaximize}
      >
        {#if $windowMaximized}<Copy />{:else}<Square />{/if}
      </button>
      <button
        class="win-btn close"
        type="button"
        use:tip={'关闭'}
        aria-label="关闭"
        on:click={closeWindow}
      >
        <X />
      </button>
    </div>
  {/if}
</div>
