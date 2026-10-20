<script lang="ts">
  // 「关于」弹窗（spec `about-page` + `app-update`）。它**不是页面**：没有页键、不切页、
  // 不参与顶部标题，两个入口（侧边栏末位的「关于」与顶部品牌行）共用 `store` 的 `aboutOpen`。
  //
  // 面板是一个固定壳（关闭键 / 品牌图形 / 产品名）+ 两个内容视图：
  // - 关于视图：版本行、技术栈、三颗外链 + 「检查更新」；
  // - 更新视图：打开即检查，按结果切换到已是最新 / 发现新版 / 下载中 / 可重启 / 失败。
  // 两个视图叠在同一栅格里；隐藏的那个脱离栅格流，避免把弹窗撑到另一视图的高度。
  // 宽高由脚本按当前视图内容定值（`animateModalHeight`），与原型同一套观感。
  //
  // 类名与版式与原型 `ui-design/launcher-prototype.html` 逐一同名（`about-*` / `update-*`）；
  // 弹出 / 关闭动画仍用 Svelte 过渡（原型用 CSS keyframes）。
  import { onDestroy, onMount, tick } from 'svelte';
  import { get } from 'svelte/store';
  import { fade, scale } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { listen } from '@tauri-apps/api/event';
  import {
    aboutOpen,
    appVersion,
    appUpdateBusy,
    compat,
    deviceCounts,
    runAppUpdateCheck,
    updateOpenIntent,
  } from '../lib/store';
  import {
    applyAppUpdate,
    cancelAppUpdate,
    openExternalLink,
    startAppUpdate,
    EVENT_APP_UPDATE_PROGRESS,
  } from '../lib/api';
  import type { AppUpdateCheck, AppUpdateProgress } from '../lib/types';
  import { copyText } from '../lib/clipboard';
  import { errText } from '../lib/install';
  import { tip } from '../lib/tooltip';
  import logoMark from '../../assets/icons/desktop/256x256.png';
  import {
    X,
    Copy,
    Check,
    Github,
    Scale,
    Globe,
    Cpu,
    FileCode,
    Shapes,
    RefreshCw,
    CircleCheck,
    CircleAlert,
    CloudDownload,
    RotateCw,
  } from 'lucide-svelte';

  /** 值未就绪时的占位：不是空值，也不是一个错误版本号。 */
  const PLACEHOLDER = '—';
  /** 复制成功反馈时长 —— 与 `LaunchBar` 的命令复制同一档（同一套反馈，不新造第二套）。 */
  const COPIED_MS = 1400;

  const reduceMotion =
    typeof window !== 'undefined' &&
    !!window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
  const fadeParams = { duration: reduceMotion ? 1 : 180 };
  const scaleParams = { duration: reduceMotion ? 1 : 200, start: 0.94, easing: cubicOut };

  $: appVer = $appVersion ?? PLACEHOLDER;
  $: scrcpyVer = $compat?.bestFit ?? PLACEHOLDER;

  // ── 复制版本号（spec「版本行的复制入口」）──
  let copiedKey = '';
  let copyTimer: ReturnType<typeof setTimeout> | null = null;
  let copyErr = '';

  async function copyVersion(key: string, text: string): Promise<void> {
    if (!text || text === PLACEHOLDER) return;
    copyErr = '';
    try {
      await copyText(text);
    } catch (e) {
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

  function onRowKeydown(e: KeyboardEvent, key: string, text: string): void {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      void copyVersion(key, text);
    }
  }

  // ── 外链：只传语义键，地址在后端 `links.rs`（全仓唯一来源）──
  let linkErr = '';

  async function openLink(link: 'repo' | 'license' | 'website'): Promise<void> {
    linkErr = '';
    try {
      await openExternalLink(link);
    } catch (e) {
      linkErr = errText(e);
    }
  }

  // ── 视图切换 ─────────────────────────────────────────────
  type View = 'about' | 'update';
  type UpdateKind =
    | 'checking'
    | 'upToDate'
    | 'installedNewer'
    | 'available'
    | 'downloading'
    | 'ready'
    | 'failed';
  type FailureKind = 'check' | 'download' | 'noAsset' | 'write';

  interface UpdateModel {
    kind: UpdateKind;
    current: string;
    latest: string;
    percent: number;
    channel: string;
    phase: 'download' | 'verify';
    failure: FailureKind | null;
  }

  const FAILURE_COPY: Record<FailureKind, { title: string; desc: string }> = {
    check: { title: '没能连上更新服务', desc: '请检查网络连接后再试一次。' },
    download: {
      title: '更新包没有下载完成',
      desc: '已尝试直连与代理通道，请稍后重试。',
    },
    noAsset: {
      title: '暂时没有可用的更新包',
      desc: '这个版本还没有适合当前系统的安装包，请稍后再试。',
    },
    write: {
      title: '当前安装位置不可写',
      desc: '无法自动替换程序文件。请把应用移到可写目录后重试，或手动下载新版本。',
    },
  };

  function initialUpdate(current: string): UpdateModel {
    return {
      kind: 'checking',
      current,
      latest: '',
      percent: 0,
      channel: '',
      phase: 'download',
      failure: null,
    };
  }

  function modelFromCheck(res: AppUpdateCheck): UpdateModel {
    const kind: UpdateKind =
      res.state === 'updateAvailable'
        ? 'available'
        : res.state === 'ready'
          ? 'ready'
          : res.state === 'installedNewer'
            ? 'installedNewer'
            : 'upToDate';
    return {
      kind,
      current: res.current,
      latest: res.latest,
      percent: 0,
      channel: '',
      phase: 'download',
      failure: null,
    };
  }

  // 打开意图由 store 传入：`check` = 手动入口，`result` = 自动检查已得到的结果。
  // 读一次即消费，避免下次从关于入口打开时残留更新视图。
  const openIntent = get(updateOpenIntent);
  updateOpenIntent.set(null);

  let view: View = openIntent ? 'update' : 'about';
  let update: UpdateModel =
    openIntent?.mode === 'result'
      ? modelFromCheck(openIntent.result)
      : initialUpdate('');
  let modalEl: HTMLDivElement;
  let updateViewEl: HTMLDivElement;
  let updateInnerEl: HTMLDivElement;
  let disposed = false;
  let downloading = false;
  let checkSeq = 0;
  let unlistenProgress: (() => void) | null = null;

  $: currentLabel = update.current || appVer;
  $: latestLabel =
    update.kind === 'checking' ? '获取中…' : update.latest || PLACEHOLDER;
  $: failureCopy = update.failure ? FAILURE_COPY[update.failure] : FAILURE_COPY.check;
  $: hasActions =
    update.kind === 'available' ||
    update.kind === 'downloading' ||
    update.kind === 'ready' ||
    update.kind === 'checking' ||
    update.kind === 'upToDate' ||
    update.kind === 'installedNewer' ||
    (update.kind === 'failed' && update.failure !== 'write');

  /**
   * 把弹窗高度从当前值平滑过渡到更新内容所需高度。
   *
   * 高度直接由 `.update-inner` 量出：不量整个面板，是因为品牌图形 / 产品名的收起过渡
   * 还在进行中，量面板会读到中间值、把更新视图留出一截空白。
   * 26 + 22 是弹窗上下内边距，1 + 1 是边框，14 是更新视图顶部内边距，最后 1px 防亚像素裁边。
   */
  async function animateModalHeight(): Promise<void> {
    await tick();
    if (!modalEl || !updateInnerEl) return;
    const next = updateInnerEl.offsetHeight + 26 + 22 + 1 + 1 + 14 + 1;
    modalEl.style.height = `${next}px`;
  }

  async function setUpdate(next: UpdateModel): Promise<void> {
    update = next;
    await animateModalHeight();
  }

  function errorKind(e: unknown, fallback: FailureKind): FailureKind {
    const kind = (e as { kind?: unknown } | null)?.kind;
    return kind === 'check' || kind === 'download' || kind === 'noAsset' || kind === 'write'
      ? kind
      : fallback;
  }

  function close(): void {
    // 下载中关闭即取消（与原型一致）；已校验完成的包由后端保留。
    if (downloading) void cancelAppUpdate().catch(() => {});
    aboutOpen.set(false);
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape' && $aboutOpen) close();
  }

  function hideImg(e: Event): void {
    (e.target as HTMLImageElement).remove();
  }

  // ── 更新流程 ─────────────────────────────────────────────

  async function applyCheck(res: AppUpdateCheck): Promise<void> {
    await setUpdate(modelFromCheck(res));
  }

  async function runCheck(): Promise<void> {
    const seq = ++checkSeq;
    const current = appVer === PLACEHOLDER ? '' : appVer;
    await setUpdate(initialUpdate(current));
    try {
      const res = await runAppUpdateCheck();
      if (disposed || seq !== checkSeq) return;
      await applyCheck(res);
    } catch (e) {
      if (disposed || seq !== checkSeq) return;
      await setUpdate({
        ...initialUpdate(current),
        kind: 'failed',
        failure: errorKind(e, 'check'),
      });
    }
  }

  /** 「检查更新」入口：先锁住关于视图的高度，再切视图并立即发起检查。 */
  async function openUpdate(): Promise<void> {
    if (modalEl) modalEl.style.height = `${modalEl.offsetHeight}px`;
    view = 'update';
    await animateModalHeight();
    updateViewEl?.focus({ preventScroll: true });
    void runCheck();
  }

  function recheck(): void {
    void runCheck();
  }

  async function beginDownload(): Promise<void> {
    if (downloading) return;
    downloading = true;
    appUpdateBusy.set(true);
    await setUpdate({ ...update, kind: 'downloading', percent: 0, channel: '', phase: 'download' });
    try {
      const outcome = await startAppUpdate();
      downloading = false;
      if (disposed) return;
      if (outcome.status === 'ready') {
        await setUpdate({
          ...update,
          kind: 'ready',
          latest: outcome.version || update.latest,
          percent: 100,
          phase: 'verify',
        });
      } else {
        close();
      }
    } catch (e) {
      downloading = false;
      if (disposed) return;
      await setUpdate({ ...update, kind: 'failed', failure: errorKind(e, 'download') });
    } finally {
      appUpdateBusy.set(false);
    }
  }

  function retry(): void {
    if (update.failure === 'download') {
      void beginDownload();
    } else {
      void runCheck();
    }
  }

  function cancelDownload(): void {
    void cancelAppUpdate().catch(() => {});
    close();
  }

  async function applyUpdate(): Promise<void> {
    appUpdateBusy.set(true);
    try {
      await applyAppUpdate();
      // exe / AppImage 会在后端重启进程；deb 已交给系统安装器，界面停在可重启。
    } catch (e) {
      if (disposed) return;
      await setUpdate({ ...update, kind: 'failed', failure: errorKind(e, 'write') });
    } finally {
      appUpdateBusy.set(false);
    }
  }

  // ── 生命周期 ─────────────────────────────────────────────

  onMount(() => {
    // 直接以更新视图打开时，首帧就按更新内容定高并把焦点移进去；
    // 预置结果不再发起第二次检查，手动入口才调用 `runCheck`。
    appUpdateBusy.set(false);
    if (view === 'update') {
      void animateModalHeight();
      updateViewEl?.focus({ preventScroll: true });
      if (openIntent?.mode === 'check') void runCheck();
    }

    let active = true;
    void listen<AppUpdateProgress>(EVENT_APP_UPDATE_PROGRESS, (e) => {
      if (!active || update.kind !== 'downloading') return;
      update = {
        ...update,
        percent: e.payload.percent,
        channel: e.payload.channel,
        phase: e.payload.phase,
      };
    }).then((unlisten) => {
      if (active) unlistenProgress = unlisten;
      else unlisten();
    });
    return () => {
      active = false;
    };
  });

  onDestroy(() => {
    disposed = true;
    if (copyTimer) clearTimeout(copyTimer);
    unlistenProgress?.();
    unlistenProgress = null;
    if (downloading) void cancelAppUpdate().catch(() => {});
  });

  let primaryLabel = '';
  $: primaryLabel =
    update.kind === 'ready' ? '立即重启并更新' : update.kind === 'available' ? '立即更新' : '';
</script>

<svelte:window on:keydown={onKeydown} />

<div class="overlay about-overlay show" transition:fade={fadeParams}>
  <div class="about-backdrop" role="presentation" on:click={close} />
  <div
    class="modal about-modal"
    class:update-mode={view === 'update'}
    role="dialog"
    aria-modal="true"
    aria-label={view === 'about' ? '关于 FreeControl2' : '检查更新'}
    bind:this={modalEl}
    transition:scale={scaleParams}
  >
    <button class="about-close" aria-label="关闭" on:click={close}><X /></button>

    <div class="about-logo"><img src={logoMark} alt="FreeControl2" on:error={hideImg} />F</div>
    <div class="about-name">FreeControl2</div>

    <div class="about-views">
      <!-- 关于视图 -->
      <div
        class="about-view"
        class:is-hidden={view === 'update'}
        aria-hidden={view === 'update'}
      >
        <div class="about-ver">
          <div
            class="about-ver-row"
            role="button"
            tabindex="0"
            use:tip={appVer === PLACEHOLDER ? '版本尚未就绪' : '复制版本号'}
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
            use:tip={scrcpyVer === PLACEHOLDER ? '版本尚未就绪' : '复制版本号'}
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

        <!-- 三颗外链 + 一颗应用内动作；「更新日志」已由「检查更新」取代。 -->
        <div class="about-links">
          <button class="btn" on:click={() => openLink('website')}><Globe />官网</button>
          <button class="btn" on:click={() => openLink('repo')}><Github />GitHub</button>
          <button class="btn" on:click={openUpdate}><RefreshCw />检查更新</button>
          <button class="btn" on:click={() => openLink('license')}><Scale />开源许可</button>
        </div>
        {#if linkErr}
          <div class="hint dl-error">{linkErr}</div>
        {/if}
        {#if copyErr}
          <div class="hint dl-error">复制失败：{copyErr}</div>
        {/if}

        <div class="about-foot">© 2026 pdone</div>
      </div>

      <!-- 更新视图 -->
      <div
        class="about-view about-view-update"
        class:is-hidden={view !== 'update'}
        aria-hidden={view !== 'update'}
        tabindex="-1"
        aria-live="polite"
        bind:this={updateViewEl}
      >
        <div class="update-inner" bind:this={updateInnerEl}>
          <div
            class="update-icon"
            class:checking={update.kind === 'checking'}
            class:ok={update.kind === 'upToDate' || update.kind === 'installedNewer' || update.kind === 'ready'}
            class:accent={update.kind === 'available' || update.kind === 'downloading'}
            class:warn={update.kind === 'failed'}
            aria-hidden="true"
          >
            {#if update.kind === 'failed'}
              <CircleAlert />
            {:else if update.kind === 'upToDate' || update.kind === 'installedNewer' || update.kind === 'ready'}
              <CircleCheck />
            {:else if update.kind === 'checking'}
              <RefreshCw />
            {:else}
              <CloudDownload />
            {/if}
          </div>

          {#if update.kind === 'checking'}
            <div class="update-title">正在检查更新…</div>
            <div class="update-desc">正在连接更新服务，请稍候。</div>
          {:else if update.kind === 'upToDate'}
            <div class="update-title">已是最新版本</div>
            <div class="update-desc">你正在使用最新版本 {update.current}。</div>
          {:else if update.kind === 'installedNewer'}
            <div class="update-title">当前版本已高于线上版本</div>
            <div class="update-desc">你正在使用 {update.current}，线上最新为 {update.latest}。</div>
          {:else if update.kind === 'available'}
            <div class="update-title">发现新版本 {update.latest}</div>
            <div class="update-desc">下载完成后重启应用即可生效。</div>
          {:else if update.kind === 'downloading'}
            <div class="update-title">正在下载新版本 {update.latest}</div>
            <div class="update-desc">下载完成后会自动校验文件完整性。</div>
          {:else if update.kind === 'ready'}
            <div class="update-title">更新已下载</div>
            <div class="update-desc">重启应用后即可使用新版本。</div>
          {:else}
            <div class="update-title">{failureCopy.title}</div>
            <div class="update-desc">{failureCopy.desc}</div>
          {/if}

          <div class="update-versions" class:is-invisible={update.kind === 'failed'}>
            <div class="update-ver"><span>当前版本</span><b>{currentLabel}</b></div>
            <div class="update-ver"><span>最新版本</span><b>{latestLabel}</b></div>
          </div>

          {#if update.kind === 'downloading'}
            <div class="update-progress">
              <div class="update-progress-head">
                <span>{update.phase === 'verify' ? '正在校验文件…' : '正在下载新版本…'}</span>
                <span>{update.phase === 'verify' ? 100 : update.percent}%</span>
              </div>
              <div class="update-progress-track">
                <div
                  class="update-progress-bar"
                  style:width={`${update.phase === 'verify' ? 100 : update.percent}%`}
                />
              </div>
              <div class="update-channel">下载通道：{update.channel || '正在连接…'}</div>
            </div>
          {/if}

          {#if hasActions}
            <div class="update-actions">
              {#if update.kind === 'available' || update.kind === 'ready'}
                <button class="btn primary" on:click={update.kind === 'available' ? beginDownload : applyUpdate}>
                  {#if update.kind === 'ready'}<RotateCw />{:else}<CloudDownload />{/if}{primaryLabel}
                </button>
              {:else if update.kind === 'downloading'}
                <button class="btn" on:click={cancelDownload}>取消下载</button>
              {:else if update.kind === 'checking'}
                <button class="btn" disabled><RefreshCw />重新检查</button>
              {:else if update.kind === 'upToDate' || update.kind === 'installedNewer'}
                <button class="btn" on:click={recheck}><RefreshCw />重新检查</button>
              {:else if update.kind === 'failed'}
                <button class="btn" on:click={retry}><RefreshCw />重试</button>
              {/if}
            </div>
          {/if}

          {#if update.kind === 'ready' && $deviceCounts.connected > 0}
            <div class="update-hint">重启会断开 {$deviceCounts.connected} 台正在投屏的设备。</div>
          {/if}
        </div>
      </div>
    </div>
  </div>
</div>
