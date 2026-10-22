<script lang="ts">
  // 「运行环境」页（design D6）：入口名、页键（`environment`）与组件文件名三者一致。
  // 页内只剩一张 `scrcpy 运行环境` 卡片，
  // 卡内依次是 目录 / 状态（含「查看原因」）/ 更新 / 下载进度与通道 / **下载通道**列表
  // —— 进度块紧跟「更新」（它是两个下载入口的即时反馈），通道列表（直连 + 代理地址）排在它之后。
  //
  // 三件事在这一页各就各位：
  // - 运行环境获取（下载通道 / 官方校验 / 原子替换，tasks 4.1–4.9）与**代理地址**同处一张卡：
  //   代理列表就是「下载通道」，与「下载最新版本」是同一件事的两半（design D9）。
  // - 「非最佳适配」的原因说明从顶栏搬到这里就地展开（design D7）—— 顶栏两个指示改为点击即跳转。
  // - 自定义参数与网络参数已移到「高级」页（`AdvancedPage.svelte`），本页只写 `channels`（代理地址），
  //   不再写 `defaults`；日志参数在「设置」页。
  //   注：本页页键 `environment` 与默认参数的**分组键**（`apps` / `advanced` 等）是两个命名空间 ——
  //   本页根本不碰 `defaults.*`，别把页键当组键去 `defaults` 里找。
  //
  // 四处字段说明分两层（spec「运行环境卡片字段说明的分层」）：字段下方一句**简短常驻说明**，
  // 细节收进标签右侧 `?` 的详情面板里按需展开（组件 `HelpTip.svelte`：无逻辑，展开全由 CSS 承担）。
  // 本轮的形态（用户 2026-10-08 定）：
  // ① 常驻说明是**为可读性新写的短句**，不再逐字沿用旧句；「状态」「下载通道」两句本来就够短，故原样。
  // ② **「目录」字段不再有常驻说明** —— 该目录路径已由只读文本框展示，再压一行只是重复；
  //    由后端下发的约定目录文字（`dirHint`）随之移进详情，详情成为它的唯一出口。
  // ③ 「更新」的常驻短句仍须含**替换后果与备份提醒**两个要点 ——「运行环境版本获取入口」要求该
  //    影响范围无需任何交互即可看到，MUST NOT 只放进详情。
  // ④ 详情可为可读性改写措辞、并按语义拆段（每段自带 `.fhelp-p`），但信息一条不丢。
  // `?` 的展开：悬浮、键盘聚焦、以及**点击后保持展开**（点击让按钮获得焦点，`:focus-within`）。
  // 不能「再点一下收起」—— 收起要点页面别处，或用键盘把焦点移走。没有 `.open`、没有 `aria-expanded`：
  // 没有脚本、也就没有展开态，「同一时刻最多一块被固定」由焦点唯一性自然成立。
  import {
    envStatus,
    compat,
    refreshEnv,
    updateScrcpyDir,
    proxies,
    addProxy,
    removeProxy,
  } from '../lib/store';
  import { openScrcpyDir, checkUpdate } from '../lib/api';
  import { installRunning, installError, runInstall } from '../lib/install';
  import { t, translateNow, type MessageKey } from '../lib/i18n';
  import { errorTextOf } from '../lib/message';
  import { open } from '@tauri-apps/plugin-dialog';
  import DownloadProgress from './DownloadProgress.svelte';
  import HelpTip from './HelpTip.svelte';
  import { tip } from '../lib/tooltip';
  import {
    Folder,
    FolderOpen,
    FolderSearch,
    ChevronDown,
    Check,
    RotateCcw,
    Download,
    RefreshCw,
    AlertTriangle,
    CircleCheck,
    Trash2,
    Plus,
    } from 'lucide-svelte';

  // ── scrcpy 运行环境状态（tasks 3.2 / 3.3 / 3.4）──
  // 就绪与否一律取后端下发的 envStatus，前端不做文件检查、不拼目录路径。
  let checking = false;
  // 瞬时反馈只存文案键（与需要拼列表的字段），渲染时才翻译：切换语言后已显示的反馈跟着变。
  interface FlashMessage {
    key: MessageKey;
    missing?: string[];
  }
  let flashMsg: FlashMessage | null = null;
  let flashCls = '';
  let flashTimer: ReturnType<typeof setTimeout> | null = null;
  $: flashText = flashMsg
    ? $t(flashMsg.key, {
        missing: (flashMsg.missing ?? []).join($t('common.commaSeparator')),
      })
    : '';

  // ── 版本获取入口（spec「运行环境版本获取入口」）──
  // 两个入口（bestFit / latest）触发后都直接开始，MUST NOT 二次确认 ——
  // 影响范围由下方「更新」字段的常驻提示承担；中止闸门是下载中的「取消」。
  // ── 检查更新（tasks 4.9）──
  let updating = false;
  type UpdateState =
    | { kind: 'idle' }
    | { kind: 'upToDate'; version: string }
    | { kind: 'available'; latest: string; installed: string }
    | { kind: 'installedNewer'; latest: string; installed: string }
    | { kind: 'notInstalled'; latest: string }
    | { kind: 'error'; error: unknown };
  let updateState: UpdateState = { kind: 'idle' };
  // ── 代理地址（tasks 4.1）──
  let newProxy = '';
  let proxyErr: unknown = null;

  async function onCheckUpdate(): Promise<void> {
    updating = true;
    updateState = { kind: 'idle' };
    try {
      const info = await checkUpdate();
      // 与「重新下载」一致：以官方最新发布版本为基准比较已安装版本（tasks 4.9）。
      if (info.state === 'upToDate') {
        updateState = { kind: 'upToDate', version: info.latest };
      } else if (info.state === 'updateAvailable') {
        updateState = {
          kind: 'available',
          latest: info.latest,
          installed: info.installed,
        };
      } else if (info.state === 'installedNewer') {
        updateState = {
          kind: 'installedNewer',
          latest: info.latest,
          installed: info.installed,
        };
      } else {
        updateState = { kind: 'notInstalled', latest: info.latest };
      }
    } catch (e) {
      updateState = { kind: 'error', error: e };
    } finally {
      updating = false;
    }
  }

  async function onAddProxy(): Promise<void> {
    proxyErr = null;
    try {
      await addProxy(newProxy);
      newProxy = '';
    } catch (e) {
      proxyErr = e;
    }
  }

  async function onRemoveProxy(url: string): Promise<void> {
    proxyErr = null;
    try {
      await removeProxy(url);
    } catch (e) {
      proxyErr = e;
    }
  }

  function updateStateText(state: UpdateState, tr: (key: MessageKey, params?: Record<string, string | number>) => string): string {
    switch (state.kind) {
      case 'upToDate':
        return tr('environment.updateUpToDate', { version: state.version });
      case 'available':
        return tr('environment.updateAvailable', {
          latest: state.latest,
          installed: state.installed,
        });
      case 'installedNewer':
        return tr('environment.updateInstalledNewer', {
          latest: state.latest,
          installed: state.installed,
        });
      case 'notInstalled':
        return tr('environment.updateNotInstalled', { latest: state.latest });
      case 'error':
        return tr('environment.updateFailed', { reason: errorTextOf(state.error, tr) });
      default:
        return '';
    }
  }

  $: updateMsg = updateStateText(updateState, $t);
  $: updateCls =
    updateState.kind === 'upToDate' || updateState.kind === 'installedNewer'
      ? 'connected'
      : updateState.kind === 'available' || updateState.kind === 'notInstalled'
        ? 'idle'
        : updateState.kind === 'error'
          ? 'offline'
          : '';

  $: ready = $envStatus?.ready ?? false;
  $: missingText = ($envStatus?.missing ?? []).join($t('common.commaSeparator'));
  // 版本号来自兼容性状态（D2：前端不得硬编码）；探测不到版本时只报「含 adb」。
  $: envText =
    $compat && $compat.direction !== 'unknown'
      ? $t('environment.ready', { version: $compat.installed })
      : $t('environment.readyNoVersion');

  // 进度块与「下载通道」那行的全部逻辑（含已耗时心跳）都在 `DownloadProgress.svelte` ——
  // 引导不再就地下载后，全仓只有本页这一个使用方。

  function setFlash(key: MessageKey, cls = '', missing?: string[]): void {
    flashMsg = { key, missing };
    flashCls = cls;
    if (flashTimer) clearTimeout(flashTimer);
    flashTimer = setTimeout(() => {
      flashMsg = null;
      flashCls = '';
    }, 2800);
  }

  async function recheck(): Promise<void> {
    checking = true;
    try {
      await refreshEnv();
      if ($envStatus?.ready) {
        setFlash('environment.rechecked', 'connected');
      } else {
        setFlash('environment.stillMissing', 'offline', $envStatus?.missing ?? []);
      }
    } catch (e) {
      console.error('重新检测失败', e);
      setFlash('environment.recheckFailed', 'offline');
    } finally {
      checking = false;
    }
  }

  // ── 版本兼容性的原因说明（spec「版本兼容性指示」，design D7）──
  //
  // 在此之前，这段说明由顶栏就地展开；顶栏两个指示改成「点击即跳转」后，原因搬到它所属的这一页：
  // 点顶栏的黄色叹号 → 到本页 → 点「查看原因」就地展开。文案、判定与两值对比全部沿用原样，
  // 只去掉原先「前往『更多』页」那个按钮 —— 用户已经在这页上了。
  $: envOk = ($envStatus?.ready ?? false) && $compat?.status === 'ok';
  let reasonOpen = false;
  // 环境恢复最佳适配（手动替换版本后重新检测 / 重新下载）时自动收起，不留过期的说明。
  $: if (envOk && reasonOpen) reasonOpen = false;

  $: installedText = $compat?.installed ?? '';
  $: bestFitText = $compat?.bestFit ?? '';
  // 方向文案与后端下发的 `direction` 一一对应 —— 前端不做版本比较（design D1）。
  $: directionKey =
    $compat?.direction === 'older'
      ? 'environment.direction.older'
      : $compat?.direction === 'newer'
        ? 'environment.direction.newer'
        : $compat?.direction === 'unknown'
          ? 'environment.direction.unknown'
          : '';
  $: directionText = directionKey ? $t(directionKey as MessageKey) : '';

  async function openDir() {
    try {
      await openScrcpyDir();
      setFlash('environment.dirOpened', 'connected');
    } catch (e) {
      console.error('打开目录失败', e);
      setFlash('environment.dirOpenFailed', 'offline');
    }
  }

  // ── 自定义目录（spec「自定义运行环境目录的设置与恢复」）──
  // 输入框是草稿：只有「应用」/ Enter / 选择目录才落盘，失焦不自动提交 ——
  // 目录是全局位置，误触失焦不该改变它。
  let dirDraft = '';
  let dirDirty = false;
  let dirErr: unknown = null;
  let dirBusy = false;
  let dirMenuOpen = false;
  let dirMenuEl: HTMLDivElement | null = null;

  // 目录行的操作收在按钮组的下拉里：点外部或按 Esc 收起。
  function closeDirMenuOnOutside(e: MouseEvent): void {
    if (dirMenuOpen && dirMenuEl && !dirMenuEl.contains(e.target as Node)) {
      dirMenuOpen = false;
    }
  }
  // 获取流程一开始就收起菜单，避免菜单里的动作在安装中被误触。
  $: if ($installRunning && dirMenuOpen) dirMenuOpen = false;

  // 后端刷新路径时同步草稿；用户正在编辑（dirDirty）时不覆盖其输入。
  $: if ($envStatus && !dirDirty && dirDraft !== $envStatus.dir) {
    dirDraft = $envStatus.dir;
  }
  $: dirChanged = !!$envStatus && dirDraft.trim() !== $envStatus.dir;

  async function applyDir(): Promise<void> {
    if ($installRunning || dirBusy) return;
    const value = dirDraft.trim();
    dirErr = null;
    dirBusy = true;
    try {
      await updateScrcpyDir(value === '' ? null : value);
      dirDirty = false;
      setFlash(value === '' ? 'environment.dirRestored' : 'environment.dirChanged', 'connected');
    } catch (e) {
      dirErr = e;
    } finally {
      dirBusy = false;
    }
  }

  async function chooseDir(): Promise<void> {
    if ($installRunning || dirBusy) return;
    dirErr = null;
    try {
      const picked = await open({
        directory: true,
        multiple: false,
        title: translateNow('environment.chooseDirTitle'),
      });
      if (typeof picked === 'string' && picked) {
        dirDraft = picked;
        dirDirty = true;
        await applyDir();
      }
    } catch (e) {
      dirErr = e;
    }
  }

  async function resetDir(): Promise<void> {
    if ($installRunning || dirBusy) return;
    dirDraft = '';
    dirDirty = true;
    await applyDir();
  }

  // `?` 的展开全由 CSS 承担（`.fhelp:hover` / `.fhelp:focus-within`，见 `app.css`）——
  // 本页不再有那套 `pinnedHelp` / `hoveredHelp` / `focusedHelp` 脚本状态。
</script>

<svelte:window
  on:click={closeDirMenuOnOutside}
  on:keydown={(e) => e.key === 'Escape' && (dirMenuOpen = false)}
/>

<div class="sec-head">
  <h2>{$t('nav.environment')}</h2>
  <p>{$t('environment.subtitle')}</p>
</div>

<div class="grid">
  <div class="card col2">
    <!-- 卡片不设标题行：本页只有这一张卡、页标题已表明身份（spec「运行环境卡片的标题呈现」）。 -->
    <div class="field">
      <span class="flabel">{$t('environment.dir')}<HelpTip title={$t('environment.dirHelpTitle')} label={$t('environment.dirHelpLabel')}>
        <span class="fhelp-p"
          >{$t('environment.dirHelp1', { hint: $envStatus?.dirHint ?? '' })}</span
        >
        <span class="fhelp-p">{$t('environment.dirHelp2')}</span>
      </HelpTip></span>
      <div class="path-row">
        <Folder />
        <input
          type="text"
          bind:value={dirDraft}
          on:input={() => {
            dirDirty = true;
            dirErr = null;
          }}
          on:keydown={(e) => e.key === 'Enter' && applyDir()}
          disabled={$installRunning || dirBusy}
          spellcheck="false"
          aria-label={$t('environment.dirAria')}
        />
        {#if dirChanged}
          <button class="btn sm" on:click={applyDir} disabled={$installRunning || dirBusy}>
            <Check /> {$t('environment.apply')}
          </button>
        {/if}
        <div class="dir-actions" bind:this={dirMenuEl}>
          <button class="btn sm" on:click={chooseDir} disabled={$installRunning || dirBusy}>
            <FolderSearch /> {$t('environment.chooseDir')}
          </button>
          <button
            class="btn sm dir-menu-trigger"
            type="button"
            aria-haspopup="menu"
            aria-expanded={dirMenuOpen}
            aria-label={$t('environment.moreDirActions')}
            on:click={() => (dirMenuOpen = !dirMenuOpen)}
            disabled={$installRunning || dirBusy}
          >
            <ChevronDown />
          </button>
          {#if dirMenuOpen}
            <div class="dir-menu-pop" role="menu">
              <button
                type="button"
                role="menuitem"
                on:click={() => {
                  dirMenuOpen = false;
                  openDir();
                }}
              >
                <FolderOpen /> {$t('environment.openDir')}
              </button>
              {#if $envStatus?.isCustom}
                <button
                  type="button"
                  role="menuitem"
                  on:click={() => {
                    dirMenuOpen = false;
                    resetDir();
                  }}
                  disabled={$installRunning || dirBusy}
                >
                  <RotateCcw /> {$t('environment.resetDir')}
                </button>
              {/if}
            </div>
          {/if}
        </div>
      </div>
      {#if dirErr}
        <div class="hint dl-error">{errorTextOf(dirErr, $t)}</div>
      {/if}
    </div>
    <div class="field">
      <span class="flabel">{$t('environment.status')}<HelpTip title={$t('environment.statusHelpTitle')} label={$t('environment.statusHelpLabel')}>
        <span class="fhelp-p">{$t('environment.statusHelp')}</span>
      </HelpTip></span>
      <div class="path-row env-state">
        {#if ready}
          <span class="pill env ok"><CircleCheck />{envText}</span>
        {:else if $envStatus}
          <span class="pill env warn"
            ><AlertTriangle />{$t('environment.notReady', { missing: missingText })}</span
          >
        {:else}
          <span class="pill env warn"><AlertTriangle />{$t('environment.checking')}</span>
        {/if}
        <button class="btn sm" on:click={recheck} disabled={checking}>
          <RefreshCw />{checking
            ? $t('environment.rechecking')
            : $t('environment.recheck')}
        </button>
        {#if ready && !envOk}
          <button
            class="btn ghost sm"
            on:click={() => (reasonOpen = !reasonOpen)}
            aria-expanded={reasonOpen}
          >
            <AlertTriangle />{reasonOpen
              ? $t('environment.hideReason')
              : $t('environment.viewReason')}
          </button>
        {/if}
        <span class="status-text {flashCls}">{flashText}</span>
      </div>
      <div class="hint">{$t('environment.statusHint')}</div>
      {#if reasonOpen}
        <div class="env-reason in-card">
          <AlertTriangle />
          <div class="env-reason-text">
            <div class="env-reason-title">{$t('environment.reasonTitle')}</div>
            <p>
              {$t('environment.reasonBodyBefore')}<strong>{$t('environment.reasonBodyBold')}</strong
              >{$t('environment.reasonBodyAfter')}
            </p>
            <div class="env-reason-cmp">
              <span>{$t('environment.installedLabel')}<b>{installedText}</b></span>
              <span>{$t('environment.bestFitLabel')}<b>{bestFitText}</b></span>
              {#if directionText}<span>{directionText}</span>{/if}
            </div>
          </div>
        </div>
      {/if}
    </div>
    <div class="field">
      <span class="flabel">{$t('environment.update')}<HelpTip title={$t('environment.updateHelpTitle')} label={$t('environment.updateHelpLabel')}>
        <span class="fhelp-p">{$t('environment.updateHelp1')}</span>
        <span class="fhelp-p">{$t('environment.updateHelp2')}</span>
        <span class="fhelp-p">{$t('environment.updateHelp3')}</span>
      </HelpTip></span>
      <!-- 两颗下载按钮的标签恒定（design D8）：「哪一颗在跑」由下方进度块说明，
           不用「获取中…」占标签 —— 两颗同时换文案反而说不清在跑的是哪颗。 -->
      <div class="path-row env-update">
        {#if !envOk}
          <!-- 最佳适配入口：仅在未就绪或版本非最佳适配时出现（spec「运行环境版本获取入口」），
               判定复用本页已有的 envOk —— 与「查看原因」同一谓词，不引入第二处版本比较。
               **不使用强调色**（2026-10-10 决定，覆盖 2026-09-23「行首并给 primary」那版设计）：
               它的可辨识性由「在场 = 当前探测到非最佳适配」这一显隐条件与行首位置承担，
               颜色留给本页真正的主操作。 -->
          <span class="tip-host" use:tip={$t('environment.downloadBestFitTip')}>
            <button class="btn sm" on:click={() => runInstall('bestFit')} disabled={$installRunning}>
              <Download /> {$t('environment.downloadBestFit')}
            </button>
          </span>
        {/if}
        <span class="tip-host" use:tip={$t('environment.downloadLatestTip')}>
          <button class="btn sm" on:click={() => runInstall('latest')} disabled={$installRunning}>
            <Download /> {$t('environment.downloadLatest')}
          </button>
        </span>
        <button
          class="btn ghost sm"
          on:click={onCheckUpdate}
          disabled={$installRunning || updating}
        >
          <RefreshCw /> {updating
            ? $t('environment.checkingUpdate')
            : $t('environment.checkUpdate')}
        </button>
        <!-- 本行的即时反馈位：**失败原因优先**（spec「运行环境版本获取入口」——失败原因就地
             呈现在触发它的这一行内），没有失败原因时才显示更新回显。失败态用 `.danger`：
             行内既有的 `.offline` 档是灰字，用它会把错误降级成提示。 -->
        {#if $installError}
          <span class="status-text danger">{errorTextOf($installError, $t)}</span>
        {:else}
          <span class="status-text {updateCls}">{updateMsg}</span>
        {/if}
      </div>
      <!-- 常驻说明只留**替换后果与备份提醒**这一句：「运行环境版本获取入口」要求该影响范围
           无需任何交互即可看到，MUST NOT 只放进 `?` 详情里；其余五句已移入详情。 -->
      <div class="hint">{$t('environment.updateHint')}</div>
    </div>
    <DownloadProgress />
    <div class="field">
      <span class="flabel">{$t('environment.channels')}<HelpTip title={$t('environment.channelsHelpTitle')} label={$t('environment.channelsHelpLabel')}>
        <span class="fhelp-p">{$t('environment.channelsHelp1')}</span>
        <span class="fhelp-p">{$t('environment.channelsHelp2')}</span>
      </HelpTip></span>
      <div class="proxy-list">
        <!-- 首行：GitHub 直连地址。它是**只读展示项**，因此不在可管理的代理列表里
             （由后端单独下发 `proxies.direct`），也不提供删除，且**没有任何点击跳转** ——
             与下面的代理行一样只是文本（同一个 `.proxy-url` 类，故呈现完全一致）。
             两处都恒为单行、超出部分省略（见 app.css 的 `.proxy-url` 规则）；被省略的那截
             不靠 title 看：`.proxy-url` 在可拖选白名单里，拖选即得全文。 -->
        <div class="proxy-item">
          <span class="tag direct">{$t('environment.direct')}</span>
          <span class="proxy-url">{$proxies.direct}</span>
          <span class="proxy-ro">{$t('environment.readOnly')}</span>
        </div>
        {#each $proxies.proxies as url (url)}
          <div class="proxy-item">
            <span class="tag proxy">{$t('environment.proxy')}</span>
            <span class="proxy-url">{url}</span>
            <button class="btn ghost sm" on:click={() => onRemoveProxy(url)}>
              <Trash2 /> {$t('environment.delete')}
            </button>
          </div>
        {/each}
        {#if $proxies.proxies.length === 0}
          <div class="proxy-empty">{$t('environment.proxyEmpty')}</div>
        {/if}
      </div>
      <div class="row proxy-add">
        <input
          type="text"
          bind:value={newProxy}
          placeholder={$t('environment.proxyPlaceholder')}
          on:keydown={(e) => e.key === 'Enter' && onAddProxy()}
        />
        <button class="btn sm" on:click={onAddProxy} disabled={!newProxy.trim()}>
          <Plus /> {$t('environment.addProxy')}
        </button>
      </div>
      {#if proxyErr}
        <div class="hint dl-error">{errorTextOf(proxyErr, $t)}</div>
      {/if}
      <div class="hint">{$t('environment.proxyHint')}</div>
    </div>
  </div>


</div>

