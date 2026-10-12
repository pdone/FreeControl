<script lang="ts">
  import { onMount } from 'svelte';
  import { t, toggleLang } from './lib/i18n';
  import Icon from './lib/Icon.svelte';
  // 构建期由 scripts/fetch-release.mjs 解析最新发行生成（.gitignore 排除）。站点源码里没有版本字面量：
  // 具体产物的直链只存在于这里带进来的取值中，取不到时一律回退到发行页。
  import release from './lib/release.json';

  const REPO = 'https://github.com/pdone/FreeControl2';
  const RELEASES = `${REPO}/releases/latest`;
  const CHANGELOG = `${REPO}/blob/main/Update.md`;
  // hero 内嵌的原型页面（构建期由 scripts/sync-assets.mjs 从 ui-design/ 镜像进站点）。
  const PREVIEW = '/preview/launcher-prototype.html';

  const rel = release as { ok: boolean; windows: string | null; appimage: string | null; deb: string | null };
  const ASSETS: Record<string, string | null> = { windows: rel.windows, appimage: rel.appimage, deb: rel.deb };
  const asset = (key: string): string => ASSETS[key] ?? RELEASES;

  // 下载一节的平台卡片里给当前系统标「你的系统」。只在客户端判一次；判不出（其它系统 / 未知 UA）就不标。
  let detectedOs = 'other';
  onMount(() => {
    const ua = navigator.userAgent;
    detectedOs = /Android/i.test(ua) ? 'other' : /Windows/i.test(ua) ? 'windows' : /Linux|X11/i.test(ua) ? 'linux' : 'other';
  });

  $: document.title = $t.metaTitle;
  $: document.documentElement.lang = $t.htmlLang;
</script>

<header class="site-header">
  <div class="wrap">
    <a class="brand" href="#top">
      <img src="/assets/icons/desktop/128x128.png" alt="FreeControl2" width="28" height="28" />
      <span>FreeControl2</span>
    </a>
    <nav class="nav">
      <a href="#features"><Icon name="LayoutGrid" size={15} />{$t.nav.features}</a>
      <a href="#compare"><Icon name="GitCompareArrows" size={15} />{$t.nav.compare}</a>
      <a href="#quickstart"><Icon name="Rocket" size={15} />{$t.nav.quickstart}</a>
      <a href="#download"><Icon name="Download" size={15} />{$t.nav.download}</a>
    </nav>
    <button class="lang-btn" type="button" on:click={toggleLang} title={$t.nav.switchTitle}>
      {$t.nav.switchLabel}
    </button>
  </div>
</header>

<main id="top">
  <!-- hero -->
  <section class="hero wrap">
    <h1>FreeControl2</h1>
    <p class="tagline">{$t.hero.tagline}</p>
    <p class="lede">{$t.hero.lede}</p>
    <div class="hero-actions">
      <a class="btn btn-primary" href={RELEASES}><Icon name="Download" />{$t.hero.downloadCta}</a>
      <a class="btn btn-ghost" href={REPO}><Icon name="Github" />{$t.hero.sourceCta}</a>
    </div>
    <!-- hero 主图是内嵌的原型页面（spec「品牌与视觉与产品同源」的 hero 预览场景）：
         原型本身即完整可操作的应用界面，读者在 hero 里就能真点。层级由 .hero-shot 提供边框/圆角/阴影。
         原型界面是中文的，与本页语言无关，故 iframe 标题按原型界面写中文。 -->
    <div class="hero-shot">
      <iframe
        class="hero-frame"
        src={PREVIEW}
        title="FreeControl2 界面预览"
        loading="lazy"
      ></iframe>
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
        <div class="get-main">
          <a class="btn btn-primary btn-xl" href={asset('windows')}>
            <Icon name="Download" size={20} />
            <span class="btn-stack"><b>{$t.download.cta}</b><small class="mono">{$t.download.osHint}</small></span>
          </a>
          <p class="get-meta">
            <a href={RELEASES}>{$t.download.allReleases}</a>
            <a href={CHANGELOG}>{$t.download.changelog}</a>
          </p>
        </div>
      </div>
      <div class="plats">
        {#each $t.download.plats as plat}
          <article class="plat" class:is-yours={plat.os === detectedOs}>
            <header>
              <Icon name={plat.icon} size={20} />
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
            <p class="plat-help">
              {#if plat.helpHref}<a href={plat.helpHref}>{plat.help}</a>{:else}{plat.help}{/if}
            </p>
          </article>
        {/each}
      </div>
      <p class="get-note">{$t.download.note}</p>
    </div>
  </section>
</main>

<footer class="site-footer">
  <div class="wrap">
    <nav class="foot-links">
      {#each $t.footer.links as link}
        <a href={link.href}><Icon name={link.icon} size={15} />{link.label}</a>
      {/each}
    </nav>
    <p>{$t.footer.builtWith}</p>
    <p>{$t.footer.license}</p>
  </div>
</footer>
