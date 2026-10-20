<script lang="ts">
  import { onMount } from 'svelte';
  import { page, collapsed, aboutOpen, initApp, windowDecorated, windowMaximized, windowTopEdgeBySystem } from './lib/store';
  import { startAutoUpdateChecks } from './lib/updateAuto';
  import Sidebar from './components/Sidebar.svelte';
  import Topbar from './components/Topbar.svelte';
  import LaunchBar from './components/LaunchBar.svelte';
  import DevicePage from './components/DevicePage.svelte';
  import DiagnosticsPage from './components/DiagnosticsPage.svelte';
  import EnvironmentPage from './components/EnvironmentPage.svelte';
  import SettingsPage from './components/SettingsPage.svelte';
  import AdvancedPage from './components/AdvancedPage.svelte';
  import ParamPage from './components/ParamPage.svelte';
  import EnvGuide from './components/EnvGuide.svelte';
  import UnverifiedConfirm from './components/UnverifiedConfirm.svelte';
  import AboutDialog from './components/AboutDialog.svelte';
  import TooltipLayer from './components/TooltipLayer.svelte';

  // 6 个纯参数页（一个 key 对应一个分组、由 `ParamPage` 铺满）。
  // 「高级」/「日志」/「运行环境」/「设置」四页各自单独渲染：它们都跨组或承载非参数字段
  // （高级页跨 apps 与 advanced 两组，日志页带会话输出，运行环境页带下载通道，
  // 设置页带应用自身的外观偏好），都不是单靠分组定义就能铺满的页面。
  //
  // 下方 section 的判据用的是**页键**（与入口名一致：`advanced` = 高级、`environment` = 运行环境），
  // 与上面提到的默认参数**分组键**（`apps` / `advanced`，属落盘配置字段）是两个命名空间 ——
  // 这里只比较 `$page`，不涉及分组。
  const paramPages = ['source', 'video', 'audio', 'control', 'record', 'window'];

  // 命令预览只在设备页与各参数页呈现：「日志」「运行环境」「设置」三页整条不呈现（含其复制入口），
  // 内容区底部也不再为它留出留白（留白档位见 `app.css` 的 `.main` / `.main.no-bar`）。
  // 判据写成"排除三页"而不是枚举八个呈现页：页键集合固定，排除式在将来新增参数页时不必同步。
  $: hasBar = $page !== 'diagnostics' && $page !== 'environment' && $page !== 'settings';

  // 屏幕缩放（设备像素比）：上沿那条边线要画出**物理** 1 像素（与系统在其余三边画的那条同宽），
  // 但它用 CSS 画、1px 是逻辑像素 —— 缩放 150% 时 1px 会落到 1.5 个物理像素，比那三条粗。
  // 故把 dpr 交给样式，由它反算高度（见 app.css 的 .win-frame-top）。
  //
  // dpr 会在窗口被拖到另一块缩放不同的显示器上时变。触发源挂两个：跟随当前 dpr 的 resolution
  // media query（渲染缩放一变它就失配）与窗口 resize（客户区尺寸一变就响）—— 那两条路径在不同
  // 实现下未必都来，任一个响了就重读一次。值没变时直接返回，免得每次 resize 都重建订阅。
  let dpr = window.devicePixelRatio;
  let dprQuery: MediaQueryList | undefined;
  const watchDpr = (): void => {
    const next = window.devicePixelRatio;
    if (next === dpr && dprQuery) return;
    dpr = next;
    dprQuery?.removeEventListener('change', watchDpr);
    dprQuery = window.matchMedia(`(resolution: ${dpr}dppx)`);
    dprQuery.addEventListener('change', watchDpr);
  };

  onMount(() => {
    // 调度器只读配置、不阻塞外壳：等配置与运行环境都就绪后再启动，
    // 避免自动弹窗与首启引导抢在同一帧。
    void initApp().finally(() => startAutoUpdateChecks());
    watchDpr();
    window.addEventListener('resize', watchDpr);
    return () => {
      window.removeEventListener('resize', watchDpr);
      dprQuery?.removeEventListener('change', watchDpr);
    };
  });
</script>

<div class="app" class:collapsed={$collapsed}>
  <Sidebar />
  <main class="main" class:no-bar={!hasBar}>
    <Topbar />
    <div class="content">
      <section class="section" class:active={$page === 'devices'}>
        <DevicePage />
      </section>

      {#each paramPages as key (key)}
        <section class="section" class:active={$page === key}>
          <ParamPage groupKey={key} />
        </section>
      {/each}

      <section class="section" class:active={$page === 'advanced'}>
        <AdvancedPage />
      </section>

      <section class="section" class:active={$page === 'diagnostics'}>
        <DiagnosticsPage />
      </section>

      <section class="section" class:active={$page === 'environment'}>
        <EnvironmentPage />
      </section>

      <section class="section" class:active={$page === 'settings'}>
        <SettingsPage />
      </section>
    </div>
    {#if hasBar}
      <LaunchBar />
    {/if}
  </main>
</div>

<!-- 首启缺失引导：模态层挂在 .app 之外（与原型一致），仅在运行环境未就绪时出现。 -->
<EnvGuide />
<!-- 「未经官方校验」的二次确认：首启引导与「运行环境」页都会用到，故挂在应用级。 -->
<UnverifiedConfirm />
<!-- 「关于」弹窗：不是页面（`about` 不在 paramPages / 也不是 section），与上面两个模态层同处。
     `{#if}` 是它对关闭动画的必要条件 —— Svelte 会等过渡的 out 播完才卸载，节点因此不会瞬隐。 -->
{#if $aboutOpen}
  <AboutDialog />
{/if}

<!-- 无装饰窗口的上沿边线（app-shell 规格「无装饰窗口的上沿边线」）：补上系统只在左、右、下
     三边画的那条边框在顶边缺的一格 —— 只在系统没在顶边画时补（Win11 系统已画了一条，再补就叠成
     2px）。三条判据全部取窗口事实（无系统装饰、未最大化、系统未在顶边绘制），不按平台名或版本号
     硬编码 —— 与自绘标题栏同一套口径。横跨整个视口（侧栏之上与右侧主界面之上都有）；样式、按
     dpr 反算高度、以及「只作呈现、不拦截指针」两条都由 app.css 的 .win-frame-top 承担。 -->
<div class="win-frame-top" class:show={!$windowDecorated && !$windowMaximized && !$windowTopEdgeBySystem} style:--dpr={dpr}></div>

<!-- hover 提示层（规格 `hover-tooltip`）：挂在应用级、以视口定位，故不受任何容器的滚动与裁剪
     影响 —— 侧边栏收起态的导航项提示正需要这一点（`.sidebar` 的 `overflow` 会把它裁掉）。
     层叠高于 `.win-frame-top` 的 60，故「关于」弹窗打开时其控件的提示仍可见。 -->
<TooltipLayer />
