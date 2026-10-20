<script lang="ts">
  import { onMount } from 'svelte';
  import { page, collapsed, aboutOpen, initApp, windowDecorated, windowMaximized } from './lib/store';
  import { startAutoUpdateChecks } from './lib/updateAuto';
  import Sidebar from './components/Sidebar.svelte';
  import Topbar from './components/Topbar.svelte';
  import LaunchBar from './components/LaunchBar.svelte';
  import DevicePage from './components/DevicePage.svelte';
  import EnvironmentPage from './components/EnvironmentPage.svelte';
  import SettingsPage from './components/SettingsPage.svelte';
  import AdvancedPage from './components/AdvancedPage.svelte';
  import ParamPage from './components/ParamPage.svelte';
  import EnvGuide from './components/EnvGuide.svelte';
  import UnverifiedConfirm from './components/UnverifiedConfirm.svelte';
  import AboutDialog from './components/AboutDialog.svelte';
  import TooltipLayer from './components/TooltipLayer.svelte';

  // 6 个纯参数页（一个 key 对应一个分组、由 `ParamPage` 铺满）。
  // 「高级」/「运行环境」/「设置」三页各自单独渲染：它们都跨组或承载非参数字段
  // （高级页跨 apps 与 advanced 两组，运行环境页带下载通道，
  // 设置页带应用自身的外观偏好与日志内容），都不是单靠分组定义就能铺满的页面。
  //
  // 下方 section 的判据用的是**页键**（与入口名一致：`advanced` = 高级、`environment` = 运行环境），
  // 与上面提到的默认参数**分组键**（`apps` / `advanced`，属落盘配置字段）是两个命名空间 ——
  // 这里只比较 `$page`，不涉及分组。
  const paramPages = ['source', 'video', 'audio', 'control', 'record', 'window'];

  // 命令预览只在设备页与各参数页呈现：「运行环境」「设置」两页整条不呈现（含其复制入口），
  // 内容区底部也不再为它留出留白（留白档位见 `app.css` 的 `.main` / `.main.no-bar`）。
  // 判据写成"排除两页"而不是枚举呈现页：页键集合固定，排除式在将来新增参数页时不必同步。
  $: hasBar = $page !== 'environment' && $page !== 'settings';

  onMount(() => {
    // 调度器只读配置、不阻塞外壳：等配置与运行环境都就绪后再启动，
    // 避免自动弹窗与首启引导抢在同一帧。
    void initApp().finally(() => startAutoUpdateChecks());
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

<!-- hover 提示层（规格 `hover-tooltip`）：挂在应用级、以视口定位，故不受任何容器的滚动与裁剪
     影响 —— 侧边栏收起态的导航项提示正需要这一点（`.sidebar` 的 `overflow` 会把它裁掉）。
     层叠高于模态遮罩 `.overlay` 的 50，故「关于」弹窗打开时其控件的提示仍可见。 -->
<TooltipLayer />
