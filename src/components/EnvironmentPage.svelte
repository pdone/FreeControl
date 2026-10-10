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
  //   不再写 `defaults`；日志参数在「日志」页。
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
    scrcpyDir,
    envStatus,
    compat,
    refreshEnv,
    proxies,
    addProxy,
    removeProxy,
  } from '../lib/store';
  import { openScrcpyDir, checkUpdate } from '../lib/api';
  import { installRunning, installError, runInstall, errText } from '../lib/install';
  import DownloadProgress from './DownloadProgress.svelte';
  import HelpTip from './HelpTip.svelte';
  import {
    Folder,
    FolderOpen,
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
  let flashMsg = '';
  let flashCls = '';
  let flashTimer: ReturnType<typeof setTimeout> | null = null;

  // ── 版本获取入口（spec「运行环境版本获取入口」）──
  // 两个入口（bestFit / latest）触发后都直接开始，MUST NOT 二次确认 ——
  // 影响范围由下方「更新」字段的常驻提示承担；中止闸门是下载中的「取消」。
  // ── 检查更新（tasks 4.9）──
  let updating = false;
  let updateMsg = '';
  let updateCls = '';
  // ── 代理地址（tasks 4.1）──
  let newProxy = '';
  let proxyErr = '';

  async function onCheckUpdate(): Promise<void> {
    updating = true;
    updateMsg = '';
    updateCls = '';
    try {
      const info = await checkUpdate();
      // 与「重新下载」一致：以官方最新发布版本为基准比较已安装版本（tasks 4.9）。
      if (info.state === 'upToDate') {
        updateMsg = `已是最新版本 v${info.latest}`;
        updateCls = 'connected';
      } else if (info.state === 'updateAvailable') {
        updateMsg = `存在新版本 v${info.latest}（当前 v${info.installed}），可点「下载最新版本」获取`;
        updateCls = 'idle';
      } else if (info.state === 'installedNewer') {
        updateMsg = `当前 v${info.installed} 高于官方最新 v${info.latest}`;
        updateCls = 'connected';
      } else {
        updateMsg = `尚未检测到已安装版本（官方最新 v${info.latest}）`;
        updateCls = 'idle';
      }
    } catch (e) {
      updateMsg = `检查更新失败：${errText(e)}`;
      updateCls = 'offline';
    } finally {
      updating = false;
    }
  }

  async function onAddProxy(): Promise<void> {
    proxyErr = '';
    try {
      await addProxy(newProxy);
      newProxy = '';
    } catch (e) {
      proxyErr = errText(e);
    }
  }

  async function onRemoveProxy(url: string): Promise<void> {
    proxyErr = '';
    try {
      await removeProxy(url);
    } catch (e) {
      proxyErr = errText(e);
    }
  }

  $: ready = $envStatus?.ready ?? false;
  $: missingText = $envStatus?.missing.join('、') ?? '';
  // 版本号来自兼容性状态（D2：前端不得硬编码）；探测不到版本时只报「含 adb」。
  $: envText =
    $compat && $compat.direction !== 'unknown'
      ? `已就绪 · scrcpy ${$compat.installed}（含 adb）`
      : '已就绪 · 含 scrcpy 与 adb';

  // 进度块与「下载通道」那行的全部逻辑（含已耗时心跳）都在 `DownloadProgress.svelte` ——
  // 引导不再就地下载后，全仓只有本页这一个使用方。

  function setFlash(text: string, cls = ''): void {
    flashMsg = text;
    flashCls = cls;
    if (flashTimer) clearTimeout(flashTimer);
    flashTimer = setTimeout(() => {
      flashMsg = '';
      flashCls = '';
    }, 2800);
  }

  async function recheck(): Promise<void> {
    checking = true;
    try {
      await refreshEnv();
      if ($envStatus?.ready) {
        setFlash('已检测到可用的运行环境', 'connected');
      } else {
        setFlash(`仍未就绪 · 缺少 ${$envStatus?.missing.join('、') ?? ''}`, 'offline');
      }
    } catch (e) {
      console.error('重新检测失败', e);
      setFlash('重新检测失败，请重试', 'offline');
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
  $: directionText = (() => {
    switch ($compat?.direction) {
      case 'older':
        return '已安装版本低于最佳适配版本（更旧）';
      case 'newer':
        return '已安装版本高于最佳适配版本（更新）';
      case 'unknown':
        return '无法探测已安装版本（scrcpy --version 与目录名解析均失败）';
      default:
        return '';
    }
  })();

  async function openDir() {
    try {
      await openScrcpyDir();
      setFlash('已在资源管理器中打开该目录', 'connected');
    } catch (e) {
      console.error('打开目录失败', e);
      setFlash('打开目录失败', 'offline');
    }
  }

  // `?` 的展开全由 CSS 承担（`.fhelp:hover` / `.fhelp:focus-within`，见 `app.css`）——
  // 本页不再有那套 `pinnedHelp` / `hoveredHelp` / `focusedHelp` 脚本状态。
</script>

<div class="sec-head">
  <h2>运行环境</h2>
  <p>管理 scrcpy 运行环境的安装目录、更新与下载通道（GitHub 直连 + 代理地址）。</p>
</div>

<div class="grid">
  <div class="card col2">
    <!-- 卡片不设标题行：本页只有这一张卡、页标题已表明身份（spec「运行环境卡片的标题呈现」）。 -->
    <div class="field">
      <span class="flabel">scrcpy 目录<HelpTip title="scrcpy 目录" label="scrcpy 目录的说明">
        <span class="fhelp-p">{#if $envStatus?.dirHint}约定目录为 {$envStatus.dirHint}；{/if}scrcpy 与 adb 都由该目录提供。</span>
        <span class="fhelp-p">首启若缺少二进制，可点「打开目录」手动放置，再点「重新检测」。</span>
      </HelpTip></span>
      <div class="path-row">
        <Folder />
        <input type="text" readonly value={$scrcpyDir} title="点击右侧按钮可在资源管理器中定位该目录" />
        <button class="btn sm" on:click={openDir} title="在资源管理器中打开该目录">
          <FolderOpen /> 打开目录
        </button>
      </div>
    </div>
    <div class="field">
      <span class="flabel">状态<HelpTip title="状态" label="状态的说明">
        <span class="fhelp-p">未就绪时仍可查看路径与打开目录，或点「下载最佳适配版本」自动获取（下载通道 / 官方校验 / 原子替换）。</span>
      </HelpTip></span>
      <div class="path-row env-state">
        {#if ready}
          <span class="pill env ok"><CircleCheck />{envText}</span>
        {:else if $envStatus}
          <span class="pill env warn"><AlertTriangle />未就绪 · 缺少 {missingText}</span>
        {:else}
          <span class="pill env warn"><AlertTriangle />检测中…</span>
        {/if}
        <button class="btn sm" on:click={recheck} disabled={checking}>
          <RefreshCw />{checking ? '检测中' : '重新检测'}
        </button>
        {#if ready && !envOk}
          <button
            class="btn ghost sm"
            on:click={() => (reasonOpen = !reasonOpen)}
            aria-expanded={reasonOpen}
            title="查看「非最佳适配版本」的原因"
          >
            <AlertTriangle />{reasonOpen ? '收起原因' : '查看原因'}
          </button>
        {/if}
        <span class="status-text {flashCls}">{flashMsg}</span>
      </div>
      <div class="hint">就绪 = 目录内 scrcpy 与 adb 齐备。</div>
      {#if reasonOpen}
        <div class="env-reason in-card">
          <AlertTriangle />
          <div class="env-reason-text">
            <div class="env-reason-title">非最佳适配版本</div>
            <p>
              当前版本非最佳适配版本，部分特性可能不受支持或存在未知问题。此提示<strong>不阻断任何操作</strong>，你仍可正常连接设备、发起会话与复制命令。
            </p>
            <div class="env-reason-cmp">
              <span>已安装 <b>{installedText}</b></span>
              <span>最佳适配 <b>{bestFitText}</b></span>
              {#if directionText}<span>{directionText}</span>{/if}
            </div>
          </div>
        </div>
      {/if}
    </div>
    <div class="field">
      <span class="flabel">更新<HelpTip title="更新" label="更新的说明">
        <span class="fhelp-p">两个入口都会删除并替换运行环境目录内的全部文件（scrcpy 与 adb 一起更新）；曾手动放入自定义版本请先备份。</span>
        <span class="fhelp-p">两者都从 GitHub Releases 获取：「下载最佳适配版本」取构建期锁定的受支持版本，不依赖官方最新发布查询；「下载最新版本」取官方最新发布，查询失败会回退到最佳适配版本并说明原因。</span>
        <span class="fhelp-p">直连失败会自动按序改用下面的代理地址，进度块会标明当前所用通道。直连虽可用但太慢时，可在进度块点「切换通道」立即换一条；下载完成后以官方 SHA256SUMS 校验，校验不通过不写入目录。</span>
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
          <button
            class="btn sm"
            on:click={() => runInstall('bestFit')}
            disabled={$installRunning}
            title="获取构建期锁定的最佳适配版本（不依赖官方最新发布查询）"
          >
            <Download /> 下载最佳适配版本
          </button>
        {/if}
        <button
          class="btn sm"
          on:click={() => runInstall('latest')}
          disabled={$installRunning}
          title="下载官方最新发布版本（会替换运行环境目录内的全部文件）"
        >
          <Download /> 下载最新版本
        </button>
        <button
          class="btn ghost sm"
          on:click={onCheckUpdate}
          disabled={$installRunning || updating}
          title="检查官方是否存在更新版本"
        >
          <RefreshCw /> {updating ? '检查中…' : '检查更新'}
        </button>
        <!-- 本行的即时反馈位：**失败原因优先**（spec「运行环境版本获取入口」——失败原因就地
             呈现在触发它的这一行内），没有失败原因时才显示更新回显。失败态用 `.danger`：
             行内既有的 `.offline` 档是灰字，用它会把错误降级成提示。 -->
        {#if $installError}
          <span class="status-text danger">{$installError}</span>
        {:else}
          <span class="status-text {updateCls}">{updateMsg}</span>
        {/if}
      </div>
      <!-- 常驻说明只留**替换后果与备份提醒**这一句：「运行环境版本获取入口」要求该影响范围
           无需任何交互即可看到，MUST NOT 只放进 `?` 详情里；其余五句已移入详情。 -->
      <div class="hint">会删除并替换目录内全部文件；自定义版本请先备份。</div>
    </div>
    <DownloadProgress />
    <div class="field">
      <span class="flabel">下载通道<HelpTip title="下载通道" label="下载通道的说明">
        <span class="fhelp-p">列表内所有地址都可删除，含出厂默认的两条；删除后不会自动恢复。</span>
        <span class="fhelp-p">追加的地址须以 http:// 或 https:// 开头，会持久化并在重启后保留。</span>
      </HelpTip></span>
      <div class="proxy-list">
        <!-- 首行：GitHub 直连地址。它是**只读展示项**，因此不在可管理的代理列表里
             （由后端单独下发 `proxies.direct`），也不提供删除，且**没有任何点击跳转** ——
             与下面的代理行一样只是文本（同一个 `.proxy-url` 类，故呈现完全一致）；
             `title` 与代理行同法承载被省略掉的全量地址。 -->
        <div class="proxy-item">
          <span class="tag direct">直连</span>
          <span class="proxy-url" title={$proxies.direct}>{$proxies.direct}</span>
          <span class="proxy-ro">只读</span>
        </div>
        {#each $proxies.proxies as url (url)}
          <div class="proxy-item">
            <span class="tag proxy">代理</span>
            <span class="proxy-url" title={url}>{url}</span>
            <button
              class="btn ghost sm"
              on:click={() => onRemoveProxy(url)}
              title="删除该代理地址"
            >
              <Trash2 /> 删除
            </button>
          </div>
        {/each}
        {#if $proxies.proxies.length === 0}
          <div class="proxy-empty">
            代理地址已全部删除，下载将只走 GitHub 直连。
          </div>
        {/if}
      </div>
      <div class="row proxy-add">
        <input
          type="text"
          bind:value={newProxy}
          placeholder="https://你的代理地址"
          on:keydown={(e) => e.key === 'Enter' && onAddProxy()}
        />
        <button class="btn sm" on:click={onAddProxy} disabled={!newProxy.trim()}>
          <Plus /> 追加
        </button>
      </div>
      {#if proxyErr}
        <div class="hint dl-error">{proxyErr}</div>
      {/if}
      <div class="hint">
        下载按列表顺序自动降级：先走 GitHub 直连，失败则依次改用下面的代理地址。
      </div>
    </div>
  </div>


</div>

