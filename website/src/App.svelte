<script lang="ts">
  import { onMount } from 'svelte';
  import { get, readable } from 'svelte/store';
  import { lang, t, toggleLang } from './lib/i18n';
  import type { Lang } from './lib/i18n';
  import { otherTheme, setThemeCaptureHook, setThemeMode, themeMode } from './lib/theme';
  import type { ThemeMode } from './lib/theme';
  import Icon from './lib/Icon.svelte';
  import { BUILT_WITH, GITHUB_MARK, OS_MARKS } from './lib/brand-marks';
  import { detectPlatform } from './lib/platform';
  import { webFontFacesCss } from './lib/web-font';
  // 构建期由 scripts/fetch-release.mjs 解析最新发行生成（.gitignore 排除）。站点源码里没有版本字面量：
  // 具体产物的直链只存在于这里带进来的取值中，取不到时一律回退到发行页。
  import release from './lib/release.json';

  const REPO = 'https://github.com/pdone/FreeControl';
  const RELEASES = `${REPO}/releases/latest`;
  // 第一代产品（更名前叫 FreeControl）—— 选型对比里那一栏指向它，避免与本产品混名。
  const LEGACY_REPO = 'https://github.com/pdone/FreeControl-Legacy';
  // hero 内嵌的原型页面（构建期由 scripts/sync-assets.mjs 从 ui-design/ 镜像进站点）。
  const PREVIEW = '/preview/launcher-prototype.html';
  // hero 窄屏分支的设备页截图（同一脚本从 assets/screenshots/ 镜像进站点；英文一套在 en/ 下，
  // 浅色一套在 light/ 下）。四张成对存在由 scripts/sync-assets.mjs 在构建前断言。
  const DEVICE_SHOT = '/screenshots/devices.png';
  const DEVICE_SHOT_EN = '/screenshots/en/devices.png';
  const DEVICE_SHOT_LIGHT = '/screenshots/light/devices.png';
  const DEVICE_SHOT_LIGHT_EN = '/screenshots/light/en/devices.png';

  // 窄屏截图的取图口径：语言 × 档位，四种组合各一张。收敛成函数，避免模板里散落三元。
  const shotPath = (l: Lang, theme: ThemeMode): string =>
    theme === 'light'
      ? l === 'en'
        ? DEVICE_SHOT_LIGHT_EN
        : DEVICE_SHOT_LIGHT
      : l === 'en'
        ? DEVICE_SHOT_EN
        : DEVICE_SHOT;

  // 外观入口的图标指向**点击后会切到的那一档**（当前浅色显示月亮＝点它变深）。
  // 与语言那颗同一套语义：控件表达"会发生什么"，而不是"现在是什么"。
  const NEXT_ICON: Record<ThemeMode, string> = { light: 'Moon', dark: 'Sun' };
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
    macosArm64: string | null;
    macosX64: string | null;
  };
  const ASSETS: Record<string, string | null> = {
    windows: rel.windows,
    windowsArm64: rel.windowsArm64,
    appimage: rel.appimage,
    deb: rel.deb,
    macosArm64: rel.macosArm64,
    macosX64: rel.macosX64,
  };
  const asset = (key: string): string => ASSETS[key] ?? RELEASES;

  // 访客平台判定（系统 + 架构）：主按钮取哪一档产物、平台卡标不标「你的系统」，都读这一处结果。
  // 客户端渲染、无 SSR，模块初始化时 window 已可用 —— 同步求值使首帧即落在正确档位（与 isWide 同法），
  // 不出现先渲 Windows 档再切到 Linux 的翻转。判定本身在 `lib/platform.ts` 里是纯函数。
  const platform = detectPlatform({
    userAgent: navigator.userAgent,
    architectures: (navigator as Navigator & { userAgentData?: { architectures?: string[] } }).userAgentData
      ?.architectures,
    maxTouchPoints: navigator.maxTouchPoints,
  });
  const detectedOs = platform.os;

  // 下载一节主按钮的取值：产物直链与那行提示由同一个档位派生 —— 提示讲哪一档，链接就指向哪一档。
  // Linux 取 AppImage（便携、通用发行版，优先于 deb）；Windows 按访客架构取 x64 或 ARM64，
  // 判不出架构时回落 x64（该产物在 ARM64 Windows 上可经仿真运行）；其余系统沿用 Windows x64 档。
  // macOS 不走这里：Safari 判不出芯片，由模板并排给出两颗按钮（Apple 芯片 / Intel）。
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

  // 预览与站点同语言：首帧由 URL 的 `?lang=` 带入（避免英文页面先闪一帧中文界面），
  // 之后页内切换只发一条消息、不重写 src —— 重写会让整份预览重新加载，把读者在预览里
  // 翻到的页面弹回设备页。iframe 后到（跨断点重建或首帧还没加载完）时由 on:load 兜一次。
  // 预览与站点同语言、同深浅，首帧都由 URL 的 `?lang=` / `?theme=` 带入（避免英文页面先闪一帧
  // 中文界面、深色站点先闪一帧浅色预览）；之后页内变更只发消息、不重写 src —— 重写会让整份
  // 预览重新加载，把读者在预览里翻到的页面弹回设备页。iframe 后到（跨断点重建或首帧还没加载
  // 完）时由 on:load 兜一次。
  // 推的是**模式**（system / light / dark）而不是生效色：站点处于「跟随系统」时，预览内部也
  // 保持"跟随系统"，它的设置页里那三档分段控件才不会显示成一个产品里不存在的固定档。
  //
  // src 只在**创建 iframe 之前**求值：跨断点重建（<1024 ↔ ≥1024）时按当时的语言与档位重拼，
  // 否则新 iframe 会先按模块初始化那一刻的语言画出首帧再被消息纠正（那正是"首帧即当前语言"
  // 要避免的闪烁）。页内切换不碰它，因此不会触发重载。
  const previewSrc = (): string => `${PREVIEW}?lang=${get(lang)}&theme=${get(themeMode)}`;
  let previewSrcValue = previewSrc();
  $: if ($isWide) previewSrcValue = previewSrc();
  let frameEl: HTMLIFrameElement | null = null;

  // 原型的 HTML 是本站最重的一件资源（约 456 KB，生产上 gzip 后仍在 100 KB 量级，比本页其它
  // 资源之和还大），而它就落在首屏内 ⇒ `loading="lazy"` 不生效，会与首屏抢带宽、并在同一渲染
  // 进程里解析排版。实测（4 倍 CPU 降速 + 10 Mbps）它的下载与解析把首次内容绘制推到 ~850 ms。
  // 做法：先只画那块空着的面（`.hero-slot` 已经撑住几何，`.hero-frame-wrap` 自带底色与描边），
  // 首屏画完后的空闲帧再把 iframe 挂上去 —— 布局零变化，预览随后照常可交互，语言同步逻辑不变。
  let previewArmed = false;
  // 原型的语言切换在自身脚本里完成；载入前不显示，避免英文访客先看到一帧中文界面。
  let previewShown = false;
  function frameGate() {
    previewShown = false;
    return { destroy() { previewShown = false; } };
  }
  onMount(() => {
    const arm = () => {
      // 挂载前再拼一次：空闲帧之前读者可能已经切过语言或档位
      previewSrcValue = previewSrc();
      previewArmed = true;
    };
    if (typeof requestIdleCallback === 'function') requestIdleCallback(arm, { timeout: 800 });
    else window.setTimeout(arm, 120);
    // 换肤过渡要等预览落地才拍"新画面"（见 lib/theme.ts 的 setThemeCaptureHook）
    setThemeCaptureHook(pushPreviewTheme);
    window.addEventListener('message', onPreviewMessage);
    // 字体可能先于 iframe 就绪（也可能晚于）；注入成功后再撤观察器。
    const fontObserver = new MutationObserver(() => {
      if (injectPreviewFont()) fontObserver.disconnect();
    });
    fontObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] });
  });

  // 放大态有三条入口，按输入能力分工，页面上不出现任何按钮：
  //   有悬停能力的输入（鼠标 / 触控板）→ 指针停留或键盘聚焦，离开即复原；
  //   触屏专用设备（CSS `(hover: none)`，见 app.css 的 .hero-tap）→ **轻点预览任意处**进入钉住态。
  //   为什么要给触屏单独一条路：iframe 会吞掉指针事件，父页面在预览区域内收不到任何事件，
  //   所以触屏那条走的是盖在预览上的一层透明覆盖层（只在 `(hover: none)` 下接指针）。
  // 退出：钉住态点遮罩（`pointer-events` 只在钉住时打开）或按 Esc。
  let pinned = false;
  $: if (!$isWide && pinned) pinned = false;

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape' && pinned) pinned = false;
  }

  function syncPreviewLang(): void {
    // 未挂载前没有可收消息的文档（挂在 about:blank 上发消息没有意义，也可能被拦）
    if (!previewArmed) return;
    frameEl?.contentWindow?.postMessage({ type: 'fc2:lang', lang: get(lang) }, location.origin);
  }

  // 与语言分开一条消息：切语言时不重推主题，免得把读者在预览设置页里自己选的那一档覆盖掉。
  //
  // 预览换肤要**回执**：站点要等它落地才让 View Transitions 拍"新画面"，这样预览的换肤
  // 正好落在两张快照之间，被同一次交叉淡入带走（否则它要么先跳再淡、要么淡完才跳）。
  // 最多等 120ms —— 预览没挂载、或跑的是不回执的旧版原型时，不让过渡卡在这里。
  // 回执按 requestId 对上这一次请求，旧回执或旧计时器不能提前放行新的等待。
  let themeAck: { id: number; finish: () => void } | null = null;
  let requestSerial = 0;

  function onPreviewMessage(e: MessageEvent): void {
    if (e.origin !== location.origin || e.source !== frameEl?.contentWindow) return;
    const data = e.data;
    if (data?.type === 'fc2:theme-applied' && themeAck?.id === data.requestId) themeAck.finish();
  }

  function pushPreviewTheme(mode: ThemeMode): Promise<void> {
    const win = frameEl?.contentWindow;
    if (!previewArmed || !win) return Promise.resolve();
    return new Promise<void>((resolve) => {
      let settled = false;
      const id = ++requestSerial;
      const finish = () => {
        if (settled) return;
        settled = true;
        if (themeAck?.id === id) themeAck = null;
        resolve();
      };
      themeAck = { id, finish };
      win.postMessage({ type: 'fc2:theme', mode, requestId: id }, location.origin);
      window.setTimeout(finish, 120);
    });
  }

  function syncPreviewTheme(): void {
    void pushPreviewTheme(get(themeMode));
  }

  // 把站点正文用的同一份字体注入预览：原型自己不带 @font-face（它必须自足、不发请求，见 config ㊲），
  // 而原型正文栈的首位已经是这个字体族，注入 @font-face 之后预览里的文字就与页面同体。
  // 站点字体未就绪时不注入，避免预览抢先触发那两份字体的下载；取不到时原型沿系统栈呈现。
  function injectPreviewFont(): boolean {
    if (!document.documentElement.classList.contains('has-web-font')) return false;
    const doc = frameEl?.contentDocument;
    if (!doc?.head) return false;
    const css = webFontFacesCss();
    if (!css) return true;
    let style = doc.getElementById('fc-web-font') as HTMLStyleElement | null;
    if (!style) {
      style = doc.createElement('style');
      style.id = 'fc-web-font';
      doc.head.appendChild(style);
    }
    if (style.textContent !== css) style.textContent = css;
    return true;
  }

  function onFrameLoad(): void {
    injectPreviewFont();
    syncPreviewLang();
    syncPreviewTheme();
    requestAnimationFrame(() => (previewShown = true));
  }

  /* ---------- 顶栏的外观入口 ---------- */
  // 一颗仅图标按钮，点击即在两档之间切换：没有菜单、没有面板、没有第二个交互面。
  // 档位的应用与整页交叉淡入都在 lib/theme.ts 里；这里只负责翻到另一档。
  function toggleAppearance(): void {
    setThemeMode(otherTheme($themeMode));
  }

  // 平台卡的光晕跟随指针（样式见 app.css 的 `.plat::before`）：只写两个自定义属性，
  // 用 rAF 合并同一帧内的多次 pointermove。不改任何布局属性，也不碰卡片内容。
  // 移出时**不清空**坐标、只把最后一帧落位：光晕在"指针离开的那一点"原地淡出。
  // （若清空，`--mx/--my` 会回落到渐变里的默认位 50% 0%，看到的是光斑先跳到卡片顶部再消失。）
  function glow(node: HTMLElement) {
    let raf = 0;
    let last: { x: number; y: number } | null = null;
    const write = () => {
      raf = 0;
      if (!last) return;
      node.style.setProperty('--mx', `${last.x}px`);
      node.style.setProperty('--my', `${last.y}px`);
    };
    const onMove = (e: PointerEvent) => {
      const r = node.getBoundingClientRect();
      last = { x: Math.round(e.clientX - r.left), y: Math.round(e.clientY - r.top) };
      if (!raf) raf = requestAnimationFrame(write);
    };
    const onLeave = () => {
      if (raf) cancelAnimationFrame(raf);
      write(); // 落位到离开前的最后一点（同一帧内可能还有一次未写的移动）
      last = null; // 之后不再更新；已写入的坐标留在元素上，作为淡出的位置
    };
    node.addEventListener('pointermove', onMove, { passive: true });
    node.addEventListener('pointerleave', onLeave);
    return {
      destroy() {
        node.removeEventListener('pointermove', onMove);
        node.removeEventListener('pointerleave', onLeave);
        if (raf) cancelAnimationFrame(raf);
      },
    };
  }

  $: if ($lang) syncPreviewLang();
</script>

<!-- 放大态的键盘出口（Esc）；按键监听常驻，但只在钉住时做事 -->
<svelte:window on:keydown={onKeydown} />

<header class="site-header">
  <!-- 顶栏内部沿用正文的全局 .wrap，宽度契约与各节一致（不做全宽特例）。
       左组＝品牌块 + 四个纯文字章节锚点；右组＝语言 / GitHub 两颗控件（顶栏不放下载入口）。
       贴右由 .head-actions 的 margin-left:auto 承载，左组因此贴左。见 spec「顶栏的构成与行为」 -->
  <div class="wrap">
    <a class="brand" href="#top">
      <img src="/assets/icons/desktop/128x128.png" alt="FreeControl" width="28" height="28" />
      <span>FreeControl</span>
    </a>
    <nav class="nav">
      <a href="#features">{$t.nav.features}</a>
      <a href="#compare">{$t.nav.compare}</a>
      <a href="#quickstart">{$t.nav.quickstart}</a>
      <a href="#faq">{$t.nav.faq}</a>
    </nav>
    <div class="head-actions">
      <!-- 外观入口：仅图标按钮，点一下就在深/浅之间切换（没有菜单、没有第二个交互面）。
           图标与可访问名都指向"点击后会切到的那一档"，与语言那颗同一套语义：
           控件表达会发生什么，而不是现在是什么。见 spec「外观入口的形态与操作」 -->
      <button
        class="appear-btn"
        type="button"
        aria-label={$themeMode === 'light' ? $t.nav.appearance.toDark : $t.nav.appearance.toLight}
        title={$themeMode === 'light' ? $t.nav.appearance.toDark : $t.nav.appearance.toLight}
        on:click={toggleAppearance}
      >
        <Icon name={NEXT_ICON[$themeMode]} size={16} />
      </button>
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
  <!-- hero 是两栏骨架（见 design D1 / D2）：左栏主张句 + 定位 + 两颗入口 + 元信息行，
       右栏是等比缩小后的可交互预览。产品名只在顶栏品牌块与本节的元信息行出现 ——
       首屏最大的字留给主张，不重复报名字。 -->
  <section class="hero wrap">
    <div class="hero-grid">
      <div class="hero-copy">
        <h1>{$t.hero.statement}</h1>
        <p class="lede">{$t.hero.lede}</p>
        <div class="hero-actions">
          <!-- hero 主按钮是页内锚点，落到「下载」一节：那里才按平台列出各发行产物的一键直链，
               读者不必为了拿产物先离开本页 -->
          <a class="btn btn-primary" href="#download"><Icon name="Download" />{$t.hero.downloadCta}</a>
          <!-- GitHub 入口的图形取 brand-marks.ts 的官方 mark（单色、继承文字色），与顶栏那颗同源 -->
          <a class="btn btn-ghost" href={REPO}><span class="gh-mark">{@html GITHUB_MARK}</span>{$t.hero.sourceCta}</a>
        </div>
        <!-- 元信息行：产品名 · 版本 · 体积 / 形态 / 许可。版本取构建期解析结果，
             解析不到时整颗不渲染（源码里没有版本字面量）。 -->
        <p class="hero-meta">
          <span class="is-brand">{$t.hero.meta[0]}</span>
          {#if rel.ok && rel.tag}
            <span class="dot" aria-hidden="true">·</span>
            <a href={RELEASES}><b class="mono">{rel.tag}</b></a>
          {/if}
          {#each $t.hero.meta.slice(1) as item}
            <span class="dot" aria-hidden="true">·</span>
            <span>{item}</span>
          {/each}
        </p>
      </div>
      <!-- 占位槽：只负责撑住第二列的宽与高。预览盒绝对定位在它上面，指针停上去时改的是
           预览盒自己的宽高（而非布局），所以放大/复原都不会让文案与下一节挪位。 -->
      <div class="hero-slot" aria-hidden="true"></div>
      <!-- 右栏预览按视口宽度二分（spec「品牌与视觉与产品同源」的 hero 预览场景）：
           够宽时内嵌原型页面 —— 原型本身即完整可操作的应用界面，读者在 hero 里就能真点；
           窄屏（手机 / 窄桌面窗口）下桌面版式压成一条反而不堪读，故改为同一页（设备页）的截图。
           窄屏不渲染 iframe，也就不加载原型。
           两种呈现都随页面语言换：原型自己有中英两套文案（由 `?lang=` 与父窗口消息定档），
           窄屏截图也按语言取对应语言的那一张；`iframe` 的名字与截图 alt 一并走字典。 -->
      <div class="hero-preview" class:is-pinned={pinned}>
        {#if $isWide}
          <div class="hero-frame-wrap">
            <!-- 空闲帧之后才挂 iframe（见脚本里的 previewArmed）：在那之前这块面是空的，
                 但几何由 .hero-slot 撑住，预览出现时页面不位移。 -->
            {#if previewArmed}
              <iframe
                class="hero-frame"
                class:is-shown={previewShown}
                src={previewSrcValue}
                title={$t.preview.frameTitle}
                loading="lazy"
                bind:this={frameEl}
                use:frameGate
                on:load={onFrameLoad}
              ></iframe>
            {/if}
            <!-- 触屏的"轻点放大"覆盖层：iframe 会吞事件，父页在预览区域内收不到指针，
                 所以用这一层接住第一下轻点。它只在 `(hover: none)` 且未钉住时接指针
                 （CSS 决定），鼠标用户完全感觉不到它的存在，原型也照常可点。 -->
            <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
            <!-- 抬起即钉住并拦掉合成 click，避免框移动后同一次轻点又落到遮罩上。 -->
            <div
              class="hero-tap"
              aria-hidden="true"
              on:touchend|nonpassive={(e) => { pinned = true; e.preventDefault(); }}
              on:click={() => (pinned = true)}
            ></div>
          </div>
        {:else}
          <div class="hero-shot-wrap">
            <!-- 窄屏下这张截图在文案之下（首屏之外），231 KB 的 PNG 没必要与首屏抢带宽：
                 交给浏览器按视口延迟取，进视口时再加载。
                 取图口径＝语言 × 生效深浅（见脚本里的 shotPath），四张由构建前断言成对存在。 -->
            <img
              class="hero-shot-img"
              src={shotPath($lang, $themeMode)}
              alt={$t.preview.shotAlt}
              loading="lazy"
              decoding="async"
            />
          </div>
        {/if}
      </div>
    </div>
    <!-- 发现性：预览下方一行提示，放在 hero 的下沿而不是压进预览框 ——
         框内那圈是原型自己的界面（右上角还有它自己的窗口控件），叠任何东西都会被读成第二个按钮。
         文案按输入能力切换（悬停 / 轻点），只靠 CSS 切，不需要判断 UA。窄屏不出现：那里是截图。 -->
    {#if $isWide}
      <p class="hero-hint">
        <span class="hero-hint-pointer">{$t.preview.hint}</span>
        <span class="hero-hint-touch">{$t.preview.hintTouch}</span>
      </p>
    {/if}
    <!-- 放大态的遮罩（样式见 app.css 的 .hero-scrim）。放在 .hero 里是为了用 :has() 跟着
         悬停/聚焦/钉住起落。常态 `pointer-events: none`（拦截指针会让悬停立刻塌），
         只有钉住态才接点击 —— 那时点它等于"还原"。 -->
    <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
    <div class="hero-scrim" aria-hidden="true" on:click={() => (pinned = false)}></div>
  </section>

  <!-- 功能特性 -->
  <section id="features" class="section">
    <div class="wrap">
      <div class="section-head">
        <h2>{$t.features.title}</h2>
        <p>{$t.features.subtitle}</p>
      </div>
      <!-- 13 条能力按四组排布（design D3）：组标题 + 条目清单 + 组尾一条等宽证据行。
           不再用同款圆角卡片 —— 每项一张卡会让这一节占去整页三分之一的高度，
           且卡片之间没有层级差；清单靠分组与细线承担结构。 -->
      <div class="feature-groups">
        {#each $t.features.groups as group}
          <section class="fg">
            <h3 class="fg-title">{group.title}</h3>
            <ul class="fg-list">
              {#each group.items as item}
                <li><b>{item.name}</b><span>{item.desc}</span></li>
              {/each}
            </ul>
            <p class="fg-evidence">{group.evidence}</p>
          </section>
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
              <th scope="col">
                <a
                  class="cmp-ext"
                  href={LEGACY_REPO}
                  target="_blank"
                  rel="noopener noreferrer">{$t.compare.columns[1]}</a>
              </th>
              <th scope="col">{$t.compare.columns[2]}</th>
            </tr>
          </thead>
          <tbody>
            {#each $t.compare.rows as row}
              <tr>
                <th scope="row">{row.aspect}</th>
                <!-- data-col 供窄屏把每个取值格排成卡片后的列名（td::before 取 attr(data-col)）：
                     值取列表头的同序文案，不另立一份列名，两种语言下自动对齐 -->
                <td class="is-self" data-col={$t.compare.columns[0]}>{row.fc2}</td>
                <td data-col={$t.compare.columns[1]}>{row.others[0]}</td>
                <td data-col={$t.compare.columns[2]}>{row.others[1]}</td>
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
      <div class="qs-lead qs-req">
        <div class="label">{$t.quickstart.runLabel}</div>
        <dl class="qs-plats">
          {#each $t.quickstart.platforms as p}
            <div>
              <dt>{p.name}</dt>
              <dd>{p.req}</dd>
            </div>
          {/each}
        </dl>
        <p class="qs-device">{$t.quickstart.device}</p>
        <p class="qs-note">{$t.quickstart.macNote}</p>
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
        <img class="get-logo" src="/assets/icons/desktop/128x128.png" alt="FreeControl" width="88" height="88" />
        <h2 class="title">{$t.download.title}</h2>
        <p class="lead">{$t.download.lead}</p>
        <!-- 主按钮按访客系统取档（见脚本里的 dlTarget/DL_HREF/DL_HINT）：Windows 取免安装单文件、
             Linux 取 AppImage，判不出者沿用 Windows。提示行与直链同出一档，不出现提示讲一套、链接指另一套。
             macOS 判不出芯片（Safari 的 UA 在 Apple 芯片上也写 Intel），故并排给出两颗，让访客自己选。 -->
        <div class="get-main">
          <div class="get-buttons">
            {#if detectedOs === 'macos'}
              <a class="btn btn-primary btn-xl" href={asset('macosArm64')}>
                <Icon name="Download" size={20} />
                <span class="btn-stack"><b>{$t.download.cta}</b><small class="mono">{$t.download.osHint.macosArm64}</small></span>
              </a>
              <a class="btn btn-primary btn-xl" href={asset('macosX64')}>
                <Icon name="Download" size={20} />
                <span class="btn-stack"><b>{$t.download.cta}</b><small class="mono">{$t.download.osHint.macosX64}</small></span>
              </a>
            {:else}
              <a class="btn btn-primary btn-xl" href={DL_HREF}>
                <Icon name="Download" size={20} />
                <span class="btn-stack"><b>{$t.download.cta}</b><small class="mono">{$t.download.osHint[DL_HINT]}</small></span>
              </a>
            {/if}
          </div>
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
          <!-- use:glow 只把指针在卡内的坐标写进 --mx/--my，样式全在 `.plat::before` 里 -->
          <article class="plat" class:is-yours={plat.os === detectedOs} use:glow>
            <header>
              <span class="plat-mark" aria-hidden="true">{@html OS_MARKS[plat.os]}</span>
              <h4>{plat.name}</h4>
              {#if plat.os === detectedOs}<span class="plat-badge">{$t.download.yours}</span>{/if}
            </header>
            <div class="pkg-rows">
              {#each plat.packages as pkg}
                <div class="pkg-row">
                  <!-- 行标签＝格式名 + 该平台的区分维度：Windows 的两份产物格式相同，架构以徽标并列
                       （`.exe` 配 `x64` / `arm64`）；Linux 的区分维度就是格式本身，不给徽标 -->
                  <div class="pkg-name">
                    <b>{pkg.format}</b>
                    {#if pkg.arch}<span class="pkg-arch">{pkg.arch}</span>{/if}
                    {#if pkg.note}<small>{pkg.note}</small>{/if}
                  </div>
                  <!-- 可见文案统一为「下载」（四份产物一致），可访问名另带所指的那一份：
                       同一张卡里两颗链接不加区分，屏幕阅读器与语音控制就分不出点的是哪个 -->
                  <a
                    class="pkg"
                    href={asset(pkg.key)}
                    aria-label={`${$t.download.dlLabel} ${pkg.arch ?? pkg.format}`}
                  ><Icon name="Download" size={14} /><span>{$t.download.dlLabel}</span></a>
                </div>
              {/each}
            </div>
            {#if plat.help}<p class="plat-help">{plat.help}</p>{/if}
          </article>
        {/each}
      </div>
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
