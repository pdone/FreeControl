<script lang="ts">
  import { readable } from 'svelte/store';
  import { t, toggleLang } from './lib/i18n';
  import Icon from './lib/Icon.svelte';
  import { BUILT_WITH, GITHUB_MARK, OS_MARKS } from './lib/brand-marks';
  import { detectPlatform } from './lib/platform';
  // 构建期由 scripts/fetch-release.mjs 解析最新发行生成（.gitignore 排除）。站点源码里没有版本字面量：
  // 具体产物的直链只存在于这里带进来的取值中，取不到时一律回退到发行页。
  import release from './lib/release.json';

  const REPO = 'https://github.com/pdone/FreeControl2';
  const RELEASES = `${REPO}/releases/latest`;
  // hero 内嵌的原型页面（构建期由 scripts/sync-assets.mjs 从 ui-design/ 镜像进站点）。
  const PREVIEW = '/preview/launcher-prototype.html';
  // hero 窄屏分支的设备页截图（同一脚本从 assets/screenshots/ 镜像进站点）。
  const DEVICE_SHOT = '/screenshots/devices.png';
  // hero 的分界：视口够宽才内嵌可交互原型；手机与窄桌面窗口改为静态截图。
  // 该阈值只在 JS 出现一次 —— CSS 不为它写媒体查询（截图取流式尺寸），避免两处数字漂移。
  const WIDE_QUERY = '(min-width: 1024px)';

  // 客户端渲染，无 SSR：初始化时即可同步取值，首帧就落在正确分支，不会先渲 iframe 再切换。
  const isWide = readable(window.matchMedia(WIDE_QUERY).matches, (set) => {
    const mql = window.matchMedia(WIDE_QUERY);
    const onChange = (e: MediaQueryListEvent) => set(e.matches);
    set(mql.matches);
    mql.addEventListener('change', onChange);
    return () => mql.removeEventListener('change', onChange);
  });

  const rel = release as {
    ok: boolean;
    tag: string | null;
    windows: string | null;
    windowsArm64: string | null;
    appimage: string | null;
    deb: string | null;
  };
  const ASSETS: Record<string, string | null> = {
    windows: rel.windows,
    windowsArm64: rel.windowsArm64,
    appimage: rel.appimage,
    deb: rel.deb,
  };
  const asset = (key: string): string => ASSETS[key] ?? RELEASES;

  // 访客平台判定（系统 + 架构）：主按钮取哪一档产物、平台卡标不标「你的系统」，都读这一处结果。
  // 客户端渲染、无 SSR，模块初始化时 window 已可用 —— 同步求值使首帧即落在正确档位（与 isWide 同法），
  // 不出现先渲 Windows 档再切到 Linux 的翻转。判定本身在 `lib/platform.ts` 里是纯函数。
  const platform = detectPlatform({
    userAgent: navigator.userAgent,
    architectures: (navigator as Navigator & { userAgentData?: { architectures?: string[] } }).userAgentData
      ?.architectures,
  });
  const detectedOs = platform.os;

  // 下载一节主按钮的取值：产物直链与那行提示由同一个档位派生 —— 提示讲哪一档，链接就指向哪一档。
  // Linux 取 AppImage（便携、通用发行版，优先于 deb）；Windows 按访客架构取 x64 或 ARM64，
  // 判不出架构时回落 x64（该产物在 ARM64 Windows 上可经仿真运行）；其余系统沿用 Windows x64 档。
  const dlTarget =
    detectedOs === 'linux'
      ? { asset: 'appimage', hint: 'linux' as const }
      : platform.arch === 'arm64'
        ? { asset: 'windowsArm64', hint: 'windowsArm64' as const }
        : { asset: 'windows', hint: 'windows' as const };
  const DL_HREF = asset(dlTarget.asset);
  const DL_HINT = dlTarget.hint;

  $: document.title = $t.metaTitle;
  $: document.documentElement.lang = $t.htmlLang;
</script>

<header class="site-header">
  <!-- 顶栏内部沿用正文的全局 .wrap，宽度契约与各节一致（不做全宽特例）。
       左组＝品牌块 + 四个纯文字章节锚点；右组＝语言 / GitHub 两颗控件（顶栏不放下载入口）。
       贴右由 .head-actions 的 margin-left:auto 承载，左组因此贴左。见 spec「顶栏的构成与行为」 -->
  <div class="wrap">
    <a class="brand" href="#top">
      <img src="/assets/icons/desktop/128x128.png" alt="FreeControl2" width="28" height="28" />
      <span>FreeControl2</span>
    </a>
    <nav class="nav">
      <a href="#features">{$t.nav.features}</a>
      <a href="#compare">{$t.nav.compare}</a>
      <a href="#quickstart">{$t.nav.quickstart}</a>
      <a href="#faq">{$t.nav.faq}</a>
    </nav>
    <div class="head-actions">
      <!-- 语言按钮：图标 + 短标签 —— 短标签承载「会切成哪种语言」这一状态，图标只作语义提示 -->
      <button class="lang-btn" type="button" on:click={toggleLang} title={$t.nav.switchTitle}>
        <Icon name="Languages" size={15} />
        <span>{$t.nav.switchLabel}</span>
      </button>
      <!-- GitHub 图标入口：仅含图标，故可访问名取两语言同值的 hero.sourceCta；图形与 hero 那颗同源 -->
      <a class="gh-btn" href={REPO} aria-label={$t.hero.sourceCta} title={$t.hero.sourceCta}>
        <span class="gh-mark">{@html GITHUB_MARK}</span>
      </a>
    </div>
  </div>
</header>

<main id="top">
  <!-- hero -->
  <section class="hero wrap">
    <h1>FreeControl2</h1>
    <p class="tagline">{$t.hero.tagline}</p>
    <p class="lede">{$t.hero.lede}</p>
    <div class="hero-actions">
      <!-- hero 主按钮是页内锚点，落到「下载」一节：那里才按平台列出各发行产物的一键直链，
           读者不必为了拿产物先离开本页 -->
      <a class="btn btn-primary" href="#download"><Icon name="Download" />{$t.hero.downloadCta}</a>
      <!-- GitHub 入口的图形取 brand-marks.ts 的官方 mark（单色、继承文字色），与顶栏那颗同源 -->
      <a class="btn btn-ghost" href={REPO}><span class="gh-mark">{@html GITHUB_MARK}</span>{$t.hero.sourceCta}</a>
    </div>
    <!-- hero 主图按视口宽度二分（spec「品牌与视觉与产品同源」的 hero 预览场景）：
         视口够宽时内嵌原型页面 —— 原型本身即完整可操作的应用界面，读者在 hero 里就能真点；
         窄屏（手机 / 窄桌面窗口）下把桌面版式压成一条反而不堪读，故改为同一页（设备页）的截图。
         窄屏不渲染 iframe，也就不加载原型。层级由 .hero-shot 提供边框/圆角/阴影。
         原型界面是中文的，与本页语言无关，故 iframe 标题与截图 alt 均按界面内容写中文。 -->
    <div class="hero-shot">
      {#if $isWide}
        <iframe
          class="hero-frame"
          src={PREVIEW}
          title="FreeControl2 界面预览"
          loading="lazy"
        ></iframe>
      {:else}
        <img class="hero-shot-img" src={DEVICE_SHOT} alt="FreeControl2 设备页" />
      {/if}
    </div>
  </section>

  <!-- 功能特性 -->
  <section id="features" class="section">
    <div class="wrap">
      <div class="section-head">
        <h2>{$t.features.title}</h2>
        <p>{$t.features.subtitle}</p>
      </div>
      <div class="feature-grid">
        {#each $t.features.items as item}
          <article class="feature-card">
            <span class="feature-ico"><Icon name={item.icon} size={18} /></span>
            <h3>{item.name}</h3>
            <p>{item.desc}</p>
          </article>
        {/each}
      </div>
    </div>
  </section>

  <!-- 选型对比：与同类工具逐项对照（取代此前的「界面」截图区）。 -->
  <section id="compare" class="section">
    <div class="wrap">
      <div class="section-head">
        <h2>{$t.compare.title}</h2>
        <p>{$t.compare.subtitle}</p>
      </div>
      <!-- 亮点三卡：先给结论，再用表逐项对照 -->
      <div class="cmp-highlights">
        {#each $t.compare.highlights as hl}
          <div class="cmp-hl">
            <span class="cmp-hl-ico"><Icon name={hl.icon} size={18} /></span>
            <b>{hl.stat}</b>
            <span class="cmp-hl-label">{hl.label}</span>
            <p>{hl.desc}</p>
          </div>
        {/each}
      </div>
      <div class="cmp-scroll">
        <table class="cmp">
          <thead>
            <tr>
              <th scope="col">{$t.compare.aspectHead}</th>
              <th scope="col" class="is-self">{$t.compare.columns[0]}</th>
              <th scope="col">{$t.compare.columns[1]}</th>
              <th scope="col">{$t.compare.columns[2]}</th>
            </tr>
          </thead>
          <tbody>
            {#each $t.compare.rows as row}
              <tr>
                <th scope="row">{row.aspect}</th>
                <td class="is-self">{row.fc2}</td>
                <td>{row.others[0]}</td>
                <td>{row.others[1]}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      <p class="cmp-note">{$t.compare.note}</p>
    </div>
  </section>

  <!-- 快速开始 -->
  <section id="quickstart" class="section">
    <div class="wrap">
      <div class="section-head">
        <h2>{$t.quickstart.title}</h2>
      </div>
      <div class="qs-lead">
        <div class="label">{$t.quickstart.runLabel}</div>
        <p>{$t.quickstart.requirements}</p>
      </div>
      <div class="qs-lead">
        <p>{$t.quickstart.portable}</p>
      </div>
      <h3 style="margin-top:28px">{$t.quickstart.stepsTitle}</h3>
      <ol class="steps">
        {#each $t.quickstart.steps as step}
          <li>
            <div>
              <b>{step.title}</b>
              <span>{step.desc}</span>
            </div>
          </li>
        {/each}
      </ol>
      <div class="build">
        <h3>{$t.quickstart.buildTitle}</h3>
        <p>{$t.quickstart.buildNote}</p>
      </div>
    </div>
  </section>

  <!-- 常见问题 -->
  <section id="faq" class="section">
    <div class="wrap">
      <div class="section-head">
        <h2>{$t.faq.title}</h2>
      </div>
      <div class="faq">
        {#each $t.faq.items as item}
          <details>
            <summary>{item.q}</summary>
            <p>{item.a}</p>
          </details>
        {/each}
      </div>
    </div>
  </section>

  <!-- 获取（下载）：版式参考 rayburst.pages.dev —— 品牌位图 + 标题 + 主按钮，下接分平台卡片。
       品牌位图取镜像来的资产包成品（与产品同源），不引入第二种强调色。每份产物给一键直下的链接
       （构建期从最新发行解析，见 fetch-release.mjs）；取不到时 asset() 回退到发行页。 -->
  <section id="download" class="section download">
    <div class="wrap">
      <div class="get-hero">
        <img class="get-logo" src="/assets/icons/desktop/128x128.png" alt="FreeControl2" width="88" height="88" />
        <h2 class="title">{$t.download.title}</h2>
        <p class="lead">{$t.download.lead}</p>
        <!-- 主按钮按访客系统取档（见脚本里的 dlTarget/DL_HREF/DL_HINT）：Windows 取免安装单文件、
             Linux 取 AppImage，判不出者沿用 Windows。提示行与直链同出一档，不出现提示讲一套、链接指另一套。 -->
        <div class="get-main">
          <a class="btn btn-primary btn-xl" href={DL_HREF}>
            <Icon name="Download" size={20} />
            <span class="btn-stack"><b>{$t.download.cta}</b><small class="mono">{$t.download.osHint[DL_HINT]}</small></span>
          </a>
          <!-- 发行信息一行：主按钮正下方、同一中线的一行小字 —— 版本在前（标签图标 + 构建期
               解析到的发行标记，整颗指最新发行页），「更新日志」随后。它紧邻按钮，但不再是胶囊
               （那会让按钮之下再并出两颗「按钮」）；发行的标记与版本号本身与语言无关，故整颗不配
               语言文案，图标只作「这是个发行标记」的提示。解析不到发行时整颗不渲染。 -->
          <p class="get-meta">
            {#if rel.ok && rel.tag}
              <a href={RELEASES}>
                <Icon name="Tag" size={14} />
                <b>{rel.tag}</b>
              </a>
            {/if}
            {#each $t.download.metaLinks as link, i}
              {#if (rel.ok && rel.tag) || i > 0}<span class="sep" aria-hidden="true">·</span>{/if}
              <a href={link.href}>{link.label}</a>
            {/each}
          </p>
        </div>
      </div>
      <div class="plats">
        {#each $t.download.plats as plat}
          <article class="plat" class:is-yours={plat.os === detectedOs}>
            <header>
              <span class="plat-mark" aria-hidden="true">{@html OS_MARKS[plat.os]}</span>
              <h4>{plat.name}</h4>
              {#if plat.os === detectedOs}<span class="plat-badge">{$t.download.yours}</span>{/if}
            </header>
            <div class="pkg-rows">
              {#each plat.packages as pkg}
                <div class="pkg-row">
                  <div class="pkg-name"><b>{pkg.format}</b>{#if pkg.note}<small>{pkg.note}</small>{/if}</div>
                  <a class="pkg" href={asset(pkg.key)}><Icon name="Download" size={14} /><span>{$t.download.dlLabel}</span></a>
                </div>
              {/each}
            </div>
            {#if plat.help}<p class="plat-help">{plat.help}</p>{/if}
          </article>
        {/each}
      </div>
      <p class="get-note">{$t.download.note}</p>
    </div>
  </section>
</main>

<footer class="site-footer">
  <!-- 两组：许可与版权居左、「构建技术」居右（详见 .foot-row）。
       导航链接不在这里 —— 「所有发行」「更新日志」已并入下载一节的发行信息一行，GitHub 在 hero 已有入口 -->
  <div class="wrap foot-row">
    <p class="foot-license">
      <a href={$t.footer.license.href}>{$t.footer.license.label}</a> · {$t.footer.copyright}
    </p>
    <!-- 「构建技术」一行：标签 + 各项目的官方 mark 与名称（mark 见 lib/brand-marks.ts，按第三方品牌原样呈现） -->
    <div class="foot-built">
      <span class="built-label">{$t.footer.builtLabel}</span>
      {#each BUILT_WITH as brand, i}
        {#if i > 0}<span class="built-dot">·</span>{/if}
        <a href={brand.href}><img src={brand.logo} alt="" width="16" height="16" />{brand.name}</a>
      {/each}
    </div>
  </div>
</footer>
