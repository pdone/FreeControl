<script lang="ts">
  // 「关于」弹窗（spec `about-page`）。它**不是页面**：没有页键、不切页、不参与顶部标题，
  // 两个入口（侧边栏末位的「关于」与顶部品牌行）共用 `store` 的 `aboutOpen`，
  // 弹窗本身挂在应用级（`App.svelte`），与首启引导 / 未校验确认同处。
  //
  // 类名与版式与原型 `ui-design/launcher-prototype.html` 逐一同名（`about-*`；药丸复用
  // `chips` / `chip`，按钮复用 `btn`，骨架复用既有 `.overlay` / `.modal`）。
  //
  // 两处实现技法与原型不同、渲染观感同值（design D10 已记账）：
  //   1. 弹出 / 关闭动画用 Svelte 的 `transition:fade` / `transition:scale`，原型是 CSS keyframes
  //      + `.closing` 类。Svelte 的过渡天然「播完 out 才移除节点」，且中途重开会打断旧过渡 ——
  //      正好对上 spec 的「先播完动画再消失」与「关闭动画中重开不卡住」，且不必手管定时器。
  //   2. 「减少动态效果」偏好下把时长压到 1ms（Svelte 不会自动处理该偏好）。
  //
  // 版本值的单一来源（spec「版本信息的单一来源」）：应用版本取 `appVersion`（来自 Tauri 的
  // `getVersion()`），适配的 scrcpy 版本取 `compat.bestFit`（后端下发）。两者一律**逐字**渲染：
  // 本文件里 MUST NOT 出现版本字面量，也不做比较或格式化（不加 `v` 前缀）。
  import { onDestroy } from 'svelte';
  import { fade, scale } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { aboutOpen, appVersion, compat } from '../lib/store';
  import { openExternalLink } from '../lib/api';
  import { copyText } from '../lib/clipboard';
  import { errText } from '../lib/install';
  // 品牌图形取资产包 desktop/ 分区的成品位图（含方底），与窗口 / 任务栏图标同一份资产，母版层次不压平涂。
  // 取 256 是为覆盖到 2.5× 缩放档（72 CSS px × 2.5 = 180 ≤ 256），该尺寸由生成流程原生产出。
  import logoMark from '../../assets/icons/desktop/256x256.png';
  import {
    X,
    Copy,
    Check,
    Github,
    FileText,
    Scale,
    Cpu,
    FileCode,
    Shapes,
  } from 'lucide-svelte';

  /** 值未就绪时的占位：不是空值，也不是一个错误版本号。 */
  const PLACEHOLDER = '—';
  /** 复制成功反馈时长 —— 与 `LaunchBar` 的命令复制同一档（同一套反馈，不新造第二套）。 */
  const COPIED_MS = 1400;

  // 减少动态效果偏好：压到 1ms（近乎瞬时），且过渡仍会走完（节点照常被移除，不会卡住）。
  const reduceMotion =
    typeof window !== 'undefined' &&
    !!window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
  const fadeParams = { duration: reduceMotion ? 1 : 180 };
  const scaleParams = { duration: reduceMotion ? 1 : 200, start: 0.94, easing: cubicOut };

  $: appVer = $appVersion ?? PLACEHOLDER;
  $: scrcpyVer = $compat?.bestFit ?? PLACEHOLDER;

  // ── 复制版本号（spec「版本行的复制入口」）：复制的是该行**显示的值**，复制到的与看到的一致 ──
  let copiedKey = '';
  let copyTimer: ReturnType<typeof setTimeout> | null = null;
  let copyErr = '';

  async function copyVersion(key: string, text: string): Promise<void> {
    // 未就绪（占位）时不写剪贴板、也不给成功反馈。
    if (!text || text === PLACEHOLDER) return;
    copyErr = '';
    try {
      await copyText(text);
    } catch (e) {
      // 失败不得停在成功态：调用方据此不给成功反馈，绝不「假装已复制」。
      copiedKey = '';
      copyErr = errText(e);
      return;
    }
    copiedKey = key;
    if (copyTimer) clearTimeout(copyTimer);
    copyTimer = setTimeout(() => {
      copiedKey = '';
      copyTimer = null;
    }, COPIED_MS);
  }

  /**
   * 整行是复制入口（不是行末那颗图标）：Enter / 空格与点击等价 ——
   * 与导航入口、品牌行、顶部两个指示同一套键盘约定，不新造第二种。
   */
  function onRowKeydown(e: KeyboardEvent, key: string, text: string): void {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      void copyVersion(key, text);
    }
  }

  onDestroy(() => {
    if (copyTimer) clearTimeout(copyTimer);
  });

  // ── 外链（spec「外链入口」）：只传语义键，地址在后端 `links.rs`（全仓唯一来源）──
  // 三颗入口共用这一条通道与同一处失败提示。
  let linkErr = '';

  async function openLink(link: 'repo' | 'changelog' | 'license'): Promise<void> {
    linkErr = '';
    try {
      await openExternalLink(link);
    } catch (e) {
      // 失败要给可见反馈，不能出现「点了没反应」。
      linkErr = errText(e);
    }
  }

  // ── 关闭（spec「弹窗的关闭方式」）：✕ / Esc / 点遮罩三种；点弹窗内部不关 ──
  function close(): void {
    aboutOpen.set(false);
  }

  /** Esc 只在本弹窗打开时生效（其它弹窗与页面输入不受影响）。 */
  function onKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape' && $aboutOpen) close();
  }

  /** 品牌图形加载失败时移除自身：与 `.about-logo:not(:has(img))` 的兜底层联动，避免破图。 */
  function hideImg(e: Event): void {
    (e.target as HTMLImageElement).remove();
  }
</script>

<svelte:window on:keydown={onKeydown} />

<!-- 弹窗骨架：遮罩（淡入淡出）+ 面板（轻微缩放上浮）。遮罩与点遮罩关闭的那一层分成两个元素：
     点遮罩的那层是纯装饰（role="presentation"），真正的 dialog 语义挂在面板上。 -->
<div class="overlay about-overlay show" transition:fade={fadeParams}>
  <!-- 点遮罩关闭：判据是「点到的就是这一层」，点弹窗内部不会走到这里 -->
  <div class="about-backdrop" role="presentation" on:click={close} />
  <div
    class="modal about-modal"
    role="dialog"
    aria-modal="true"
    aria-label="关于 FreeControl2"
    transition:scale={scaleParams}
  >
    <button class="about-close" title="关闭" aria-label="关闭" on:click={close}><X /></button>

    <!-- 品牌图形与侧边栏品牌行、窗口图标同源（同一母版派生的成品位图，各按显示尺寸取用） -->
    <div class="about-logo"><img src={logoMark} alt="FreeControl2" on:error={hideImg} />F</div>
    <div class="about-name">FreeControl2</div>

    <!-- 两行版本行等宽对齐；值逐字取自来源，界面不改写。
         **整行是复制入口**（不是行末那颗图标）：行可点、可键盘聚焦，Enter / 空格与点击等价；
         行末图标是反馈载体（成功后转 ✓、1.4 秒复原），自身不可点 —— 一行只有一个入口。 -->
    <div class="about-ver">
      <div
        class="about-ver-row"
        role="button"
        tabindex="0"
        title={appVer === PLACEHOLDER ? '版本尚未就绪' : '复制版本号'}
        aria-label={copiedKey === 'app' ? '已复制应用版本号' : '复制应用版本号'}
        on:click={() => copyVersion('app', appVer)}
        on:keydown={(e) => onRowKeydown(e, 'app', appVer)}
      >
        <span class="about-ver-name">FreeControl2<span class="about-ver-tag">版本</span></span>
        <span class="about-ver-val">{appVer}</span>
        <span class="about-copy" class:ok={copiedKey === 'app'} aria-hidden="true">
          {#if copiedKey === 'app'}<Check />{:else}<Copy />{/if}
        </span>
      </div>
      <div
        class="about-ver-row"
        role="button"
        tabindex="0"
        title={scrcpyVer === PLACEHOLDER ? '版本尚未就绪' : '复制版本号'}
        aria-label={copiedKey === 'scrcpy' ? '已复制 scrcpy 版本号' : '复制 scrcpy 版本号'}
        on:click={() => copyVersion('scrcpy', scrcpyVer)}
        on:keydown={(e) => onRowKeydown(e, 'scrcpy', scrcpyVer)}
      >
        <span class="about-ver-name">scrcpy<span class="about-ver-tag">适配版本</span></span>
        <span class="about-ver-val">{scrcpyVer}</span>
        <span class="about-copy" class:ok={copiedKey === 'scrcpy'} aria-hidden="true">
          {#if copiedKey === 'scrcpy'}<Check />{:else}<Copy />{/if}
        </span>
      </div>
    </div>

    

    <div class="about-chips-title">技术栈</div>
    <div class="chips">
      <span class="chip"><Cpu />Rust + Tauri 2</span>
      <span class="chip"><FileCode />Svelte + Vite + TypeScript</span>
      <span class="chip"><Shapes />lucide 图标</span>
    </div>

    <!-- 三颗外链入口（spec「外链入口」）：地址是后端的事实，前端只给语义键。
         落位由两列栅格按 DOM 顺序自动排 —— 第一行 GitHub / 更新日志，第二行只有左下的
         「开源许可」（即 GitHub 正下方），右下那格留空是验收过的取值。
         「官网」与「支持」已撤除，地址表与语义键类型里都不再有它们。 -->
    <div class="about-links">
      <button class="btn" on:click={() => openLink('repo')}><Github />GitHub</button>
      <button class="btn" on:click={() => openLink('changelog')}><FileText />更新日志</button>
      <button class="btn" on:click={() => openLink('license')}><Scale />开源许可</button>
    </div>
    <!-- 失败原因直接取自后端/剪贴板实现，它们已经是可读的一句话，不再套一层前缀 -->
    {#if linkErr}
      <div class="hint dl-error">{linkErr}</div>
    {/if}
    {#if copyErr}
      <div class="hint dl-error">复制失败：{copyErr}</div>
    {/if}

    <!-- 页脚只有这一行版权（spec「页脚版权」）：属这个弹窗自身，MUST NOT 成为全局页脚。 -->
    <div class="about-foot">© 2026 pdone</div>
  </div>
</div>
