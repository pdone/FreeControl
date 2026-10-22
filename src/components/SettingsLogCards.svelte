<script lang="ts">
  // 设置页的日志内容卡（design D1 / D2）：由原「日志」页迁入设置页的同一个栅格。
  // 三张卡的卡内排布不变；「运行日志参数」「日志文件」同排在前，「会话输出」独占整行放在页面底部
  // —— 会话输出的高度随会话与日志行数变化，放最后不会顶开上方设置。
  //
  // **页 key ≠ 组 key**：本组件不再有页 key；它承载的 3 个 flag 在数据上仍属 `advanced` 组
  // （`defaults.advanced`），因此 `config.json` 结构、`command.ts` 的拼装、`validate.ts` 的校验
  // 都没变；错误键也仍按 `advanced` 取。别到 `defaults` 里去找本组件的键，它不存在。
  //
  // 会话输出不是参数（没有字段、不进字段模型），故由本组件自行渲染。每个会话的输出由后端
  // 保存在内存里（**已结束的会话同样保留**，异常退出后的输出正是排查依据）；前端只做展示。
  // 运行中的会话每 2 秒自动取一次最新输出，省掉反复手动刷新。
  import { onDestroy, onMount, tick } from 'svelte';
  import {
    defaults,
    defaultsErrors,
    config,
    scheduleSave,
    sessions,
    selectedSessionId,
    revealSessionOutput,
    sessionLog,
    clearSessionLog,
    disconnectDevice,
  } from '../lib/store';
  import { openLogDir } from '../lib/api';
  import {
    t,
    type MessageKey,
    type MessageParams,
  } from '../lib/i18n';
  import { errorTextOf } from '../lib/message';
  import ParamField from './ParamField.svelte';
  import HelpTip from './HelpTip.svelte';
  import { ADVANCED_GROUP, RUN_LOG_CARD, refKey } from '../lib/params';
  import type { SessionInfo } from '../lib/types';
  import { FolderOpen, ZapOff, RefreshCw, Eraser } from 'lucide-svelte';
  import { tip } from '../lib/tooltip';

  type GroupValues = Record<string, unknown>;
  type GroupMap = Record<string, GroupValues>;

  // ── 日志文件（log-persistence）：保留策略 + 打开日志目录 ──
  //
  // 保留策略**不是** scrcpy 参数：它不进 `params.ts`、没有 `defaults.*` 归属，也没有对应的
  // flag —— 它是配置里的一项（后端 `config.logging.retention`）。因此本卡片自行渲染，
  // 写回走 `config.update` + `scheduleSave()`（与同组件 `updateAdv` 同一套）。
  // 取值须与后端 `config::RETENTION_*` 一致；未知取值由后端按出厂默认处理。
  const RETENTIONS: Array<{ value: string; labelKey: MessageKey }> = [
    { value: '7d', labelKey: 'settings.log.retain7' },
    { value: '30d', labelKey: 'settings.log.retain30' },
    { value: 'forever', labelKey: 'settings.log.retainForever' },
  ];
  /** 配置尚未就绪时的渲染值（出厂默认）。**不是**第二份默认值来源：落盘值仍由后端下发的 config 决定。 */
  const RETENTION_PENDING = '7d';

  // 提示只存文案键、错误只存消息对象：渲染时才翻译，切换语言后已显示的反馈跟着变。
  let logDirMsg: MessageKey | null = null;
  let logDirCls = '';
  let logDirTimer: ReturnType<typeof setTimeout> | null = null;

  $: retention = $config?.logging?.retention ?? RETENTION_PENDING;

  function setRetention(value: string): void {
    if (value === retention) return;
    config.update((c) =>
      c ? { ...c, logging: { ...c.logging, retention: value } } : c,
    );
    scheduleSave();
  }

  /** 打开日志目录：与运行环境页「打开目录」同一套就地反馈。 */
  async function openLogDirNow(): Promise<void> {
    try {
      await openLogDir();
      flashLogDir('settings.log.dirOpened', 'connected');
    } catch (e) {
      console.error('打开日志目录失败', e);
      flashLogDir('settings.log.dirFailed', 'offline');
    }
  }

  function flashLogDir(key: MessageKey, cls: string): void {
    logDirMsg = key;
    logDirCls = cls;
    if (logDirTimer) clearTimeout(logDirTimer);
    logDirTimer = setTimeout(() => {
      logDirMsg = null;
      logDirCls = '';
    }, 2800);
  }

  // ── 运行日志参数（defaults.advanced 下的日志键）──
  //
  // 写回时**先合入本地的 `advanced` 副本再整体写回**：本组件只改日志这几个键，不能把同组内
  // 由「高级」页编辑的自定义参数 / 网络键一并覆盖 —— 去抖窗口内它们可能已有未落盘的改动。
  let adv: GroupValues = {};
  $: {
    const d = $defaults as unknown as GroupMap;
    if (d && d.advanced) adv = { ...d.advanced };
  }

  function updateAdv(key: string, v: unknown) {
    adv = { ...adv, [key]: v };
    config.update((c) => {
      if (!c) return c;
      const cur = (c.defaults as unknown as GroupMap) ?? {};
      const nextDefaults = { ...cur, advanced: { ...adv } } as unknown as typeof c.defaults;
      return { ...c, defaults: nextDefaults };
    });
    scheduleSave();
  }

  // ── 会话输出与诊断（tasks 9.4）──────────────────────────────
  let logDeviceId: string | null = null;
  let logText = '';
  let logErr: unknown = null;
  let sessErr: unknown = null;
  let logTimer: ReturnType<typeof setInterval> | null = null;
  let sessionCardEl: HTMLDivElement;
  // 清空的代次：每清一次自增，让「清空请求发出前就已在途」的那次取输出结果作废 ——
  // 否则它回来会把刚清掉的行原样写回界面（自动刷新 2 秒一轮，撞上是常态而非偶然）。
  let logGen = 0;

  $: sessionList = $sessions;
  // 未显式选择时回落到最近一条会话（列表按启动时间倒序，故 [0] 就是最近的那条）。
  $: selSession =
    sessionList.find((s) => s.deviceId === $selectedSessionId) ?? sessionList[0] ?? null;
  // 选中会话并读取当前语言：`$t` 出现在这里，语言切换时这条摘要会跟着重算。
  $: selectedMeta = selSession ? sessionMeta(selSession, $t) : '';
  // 选中的会话变了就立刻取一次输出（不等下一轮自动刷新）。
  $: if (selSession) selectLogFor(selSession.deviceId);

  /** 切换要查看的会话；同一条则不重复取。 */
  function selectLogFor(deviceId: string): void {
    if (deviceId === logDeviceId) return;
    logDeviceId = deviceId;
    void loadLog(deviceId);
  }

  async function loadLog(deviceId: string): Promise<void> {
    const gen = logGen;
    try {
      const text = await sessionLog(deviceId);
      // 期间用户可能已切到别的会话，或清空过输出：两种「过期」都丢弃，别把旧行写回界面。
      if (deviceId !== logDeviceId || gen !== logGen) return;
      logText = text;
      logErr = null;
    } catch (e) {
      if (deviceId !== logDeviceId || gen !== logGen) return;
      logErr = e;
    }
  }

  function refreshLog(): void {
    if (logDeviceId) void loadLog(logDeviceId);
  }

  /**
   * 清空当前这条会话已保留的输出（后端真删）。
   * 成功才清视图 —— 请求失败时界面继续显示原有输出，与后端的实际状态一致。
   */
  async function clearLog(): Promise<void> {
    const deviceId = logDeviceId;
    if (!deviceId) return;
    logGen += 1;
    try {
      await clearSessionLog(deviceId);
      if (deviceId === logDeviceId) {
        logText = '';
        logErr = null;
      }
    } catch (e) {
      logErr = e;
    }
  }

  onMount(() => {
    logTimer = setInterval(refreshLog, 2000);
  });
  onDestroy(() => {
    if (logTimer) clearInterval(logTimer);
    if (logDirTimer) clearTimeout(logDirTimer);
  });

  /** 设备卡「日志」入口的一次性定位：切页完成后把会话输出卡滚到内容区顶部。 */
  $: if ($revealSessionOutput) void revealSessionCard();

  async function revealSessionCard(): Promise<void> {
    revealSessionOutput.set(false);
    await tick();
    const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    sessionCardEl?.scrollIntoView({
      block: 'start',
      behavior: reduce ? 'auto' : 'smooth',
    });
  }

  /** 查看某条会话的输出。 */
  function pickSession(deviceId: string): void {
    sessErr = null;
    selectedSessionId.set(deviceId);
  }

  /** 从设置页的会话输出区直接结束某个会话（排查异常时不必回设备页）。 */
  async function stopSession(deviceId: string): Promise<void> {
    sessErr = null;
    try {
      await disconnectDevice(deviceId);
    } catch (e) {
      sessErr = e;
    }
  }

  type Translate = (key: MessageKey, params?: MessageParams) => string;

  /**
   * 会话的一行说明：启动时间 + 窗口位置。
   * 位置说明与设备页同源（自动排列 / 让位 / 空间不足降级，tasks 9.6 / 9.8 / 9.9）。
   */
  function sessionMeta(s: SessionInfo, tr: Translate): string {
    const time = new Date(s.startedAt).toLocaleTimeString();
    // 「未参与自动排列」的两种原因各用各的措辞（design D5）：平台不支持时说明后果。
    const win = !s.arranged
      ? tr(
          s.arrangeSkip === 'platform'
            ? 'settings.sessions.arrangePlatform'
            : 'settings.sessions.arrangeUser',
        )
      : s.slotReused
        ? tr('settings.sessions.windowReused', { x: s.windowX ?? '', y: s.windowY ?? '' })
        : tr('settings.sessions.window', { x: s.windowX ?? '', y: s.windowY ?? '' });
    return tr('settings.sessions.startedAt', { time, win, lines: s.logLines });
  }
</script>

<!-- 运行日志参数与日志文件同排在前：两张卡内容都不多，上下叠放会留下大片空白；
     会话输出放在页面底部，其不定高度不会顶开这两张卡。 -->
<div class="card">
  <h3>{$t(RUN_LOG_CARD.titleKey)}</h3>
  {#each RUN_LOG_CARD.fields as f (f.key)}
    <ParamField
      field={f}
      value={adv[f.key]}
      error={$defaultsErrors[refKey(ADVANCED_GROUP.key, f.key)] ?? null}
      on:change={(e) => updateAdv(f.key, e.detail)}
    />
  {/each}
</div>

<!-- 日志文件与运行日志参数同排、各占一列；会话输出在其下、独占整行。
     卡内不展示日志目录路径与当前日志文件名 —— 查看位置由「打开日志目录」承担。 -->
<div class="card">
  <h3>{$t('settings.log.title')}</h3>
  <!-- 「打开日志目录」与保留策略同处一行：两者是同一件事的两半（这份日志留在哪、留多久），
       而按钮的文字已经说明它去哪 —— 不必再有一个「日志目录」标签把同一件事说第二遍。
       这一行复用既有的 `.path-row`（本仓「一颗按钮 + 就地反馈文字」的横排类）；
       命名与清理口径收进标签旁的 `?`，不再常驻在卡片下方。 -->
  <div class="field">
    <span class="flabel"
      >{$t('settings.log.retention')}<HelpTip title={$t('settings.log.helpTitle')} label={$t('settings.log.helpLabel')}>
        <span class="fhelp-p"
          >{$t('settings.log.helpBefore')}<code>yyyy-MM-dd.log</code>{$t('settings.log.helpAfter')}</span
        >
      </HelpTip></span
    >
    <div class="path-row log-file-row">
      <div class="seg">
        {#each RETENTIONS as r (r.value)}
          <button class:active={retention === r.value} on:click={() => setRetention(r.value)}
            >{$t(r.labelKey)}</button
          >
        {/each}
      </div>
      <button class="btn sm" on:click={openLogDirNow}>
        <FolderOpen /> {$t('settings.log.openDir')}
      </button>
      <span class="status-text {logDirCls}">{logDirMsg ? $t(logDirMsg) : ''}</span>
    </div>
  </div>
</div>

<!-- 会话输出独占整行，放在页面底部（design D1）：每个会话的输出都留存下来，用于排查异常退出。 -->
<div class="card col2" bind:this={sessionCardEl}>
  <div class="field">
    <span class="flabel">{$t('settings.sessions.title')}</span>
    {#if sessionList.length === 0}
      <div class="ov-empty">{$t('settings.sessions.empty')}</div>
    {:else}
      <div class="sess-list">
        {#each sessionList as s (s.deviceId)}
          <div class="sess-item" class:sel={selSession?.deviceId === s.deviceId}>
            <button
              class="sess-pick"
              on:click={() => pickSession(s.deviceId)}
              use:tip={$t('settings.sessions.view')}
            >
              <span class="status-dot {s.running ? 'connected' : 'idle'}"></span>
              <span class="sess-name">{s.alias}</span>
              <span class="sess-state">
                {s.running
                  ? $t('settings.sessions.running', { pid: s.pid })
                  : $t('settings.sessions.ended', { code: s.exitCode ?? '-' })}
              </span>
            </button>
            {#if s.running}
              <button
                class="btn ghost sm"
                on:click={() => stopSession(s.deviceId)}
              >
                <ZapOff /> {$t('settings.sessions.disconnect')}
              </button>
            {/if}
          </div>
        {/each}
      </div>

      {#if selSession}
        <div class="sess-cmd">
          <code>{selSession.command}</code>
        </div>
        <div class="sess-meta">{selectedMeta}</div>
        <pre class="log-view">{logText || $t('settings.sessions.noOutput')}</pre>
        <div class="log-actions">
          <button
            class="btn ghost sm"
            on:click={refreshLog}
          >
            <RefreshCw /> {$t('settings.sessions.refresh')}
          </button>
          <button
            class="btn ghost sm"
            on:click={clearLog}
            use:tip={$t('settings.sessions.clearTip')}
          >
            <Eraser /> {$t('settings.sessions.clear')}
          </button>
          <span class="hint">{$t('settings.sessions.autoRefresh')}</span>
        </div>
      {/if}
    {/if}
    {#if sessErr}
      <div class="hint dl-error">{errorTextOf(sessErr, $t)}</div>
    {/if}
    {#if logErr}
      <div class="hint dl-error">{errorTextOf(logErr, $t)}</div>
    {/if}
  </div>
</div>
