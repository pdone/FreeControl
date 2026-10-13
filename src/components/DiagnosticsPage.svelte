<script lang="ts">
  // 「日志」页（design D1 / D2 / D5）：从「运行环境」页提为侧边栏独立入口，
  // 承载「会话输出」与「运行日志参数」两张卡片，**输出在上、参数在下**（排查时先看输出）。
  //
  // **页 key ≠ 组 key**：本页 key 是 `diagnostics`，而它承载的 3 个 flag 在数据上仍属
  // `advanced` 组（`defaults.advanced`）—— 页只换呈现位置，不换存储归属。因此 `config.json`
  // 结构、`command.ts` 的拼装、`validate.ts` 的校验都没变；错误键也仍按 `advanced` 取。
  // 别到 `defaults` 里去找 `diagnostics` 这个键，它不存在。
  //
  // 会话输出不是参数（没有字段、不进字段模型），故由本页自行渲染。每个会话的输出由后端
  // 保存在内存里（**已结束的会话同样保留**，异常退出后的输出正是排查依据）；前端只做展示。
  // 运行中的会话每 2 秒自动取一次最新输出，省掉反复手动刷新。
  import { onDestroy, onMount } from 'svelte';
  import {
    defaults,
    defaultsErrors,
    config,
    scheduleSave,
    sessions,
    selectedSessionId,
    sessionLog,
    clearSessionLog,
    disconnectDevice,
  } from '../lib/store';
  import { errText } from '../lib/install';
  import { openLogDir } from '../lib/api';
  import ParamField from './ParamField.svelte';
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
  // 写回走 `config.update` + `scheduleSave()`（与同页 `updateAdv` 同一套）。
  // 取值须与后端 `config::RETENTION_*` 一致；未知取值由后端按出厂默认处理。
  const RETENTIONS = [
    { value: '7d', label: '保留 7 天' },
    { value: '30d', label: '保留 30 天' },
    { value: 'forever', label: '保留永久' },
  ];
  /** 配置尚未就绪时的渲染值（出厂默认）。**不是**第二份默认值来源：落盘值仍由后端下发的 config 决定。 */
  const RETENTION_PENDING = '7d';

  let logDirMsg = '';
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
      flashLogDir('已在资源管理器中打开该目录', 'connected');
    } catch (e) {
      console.error('打开日志目录失败', e);
      flashLogDir('打开日志目录失败', 'offline');
    }
  }

  function flashLogDir(text: string, cls: string): void {
    logDirMsg = text;
    logDirCls = cls;
    if (logDirTimer) clearTimeout(logDirTimer);
    logDirTimer = setTimeout(() => {
      logDirMsg = '';
      logDirCls = '';
    }, 2800);
  }

  // ── 运行日志参数（defaults.advanced 下的日志键）──
  //
  // 写回时**先合入本地的 `advanced` 副本再整体写回**：本页只改日志这几个键，不能把同组内
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
  let logErr = '';
  let sessErr = '';
  let logTimer: ReturnType<typeof setInterval> | null = null;
  // 清空的代次：每清一次自增，让「清空请求发出前就已在途」的那次取输出结果作废 ——
  // 否则它回来会把刚清掉的行原样写回界面（自动刷新 2 秒一轮，撞上是常态而非偶然）。
  let logGen = 0;

  $: sessionList = $sessions;
  // 未显式选择时回落到最近一条会话（列表按启动时间倒序，故 [0] 就是最近的那条）。
  $: selSession =
    sessionList.find((s) => s.deviceId === $selectedSessionId) ?? sessionList[0] ?? null;
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
      logErr = '';
    } catch (e) {
      if (deviceId !== logDeviceId || gen !== logGen) return;
      logErr = errText(e);
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
        logErr = '';
      }
    } catch (e) {
      logErr = errText(e);
    }
  }

  onMount(() => {
    logTimer = setInterval(refreshLog, 2000);
  });
  onDestroy(() => {
    if (logTimer) clearInterval(logTimer);
    if (logDirTimer) clearTimeout(logDirTimer);
  });

  /** 查看某条会话的输出。 */
  function pickSession(deviceId: string): void {
    sessErr = '';
    selectedSessionId.set(deviceId);
  }

  /** 从日志页直接结束某个会话（排查异常时不必回设备页）。 */
  async function stopSession(deviceId: string): Promise<void> {
    sessErr = '';
    try {
      await disconnectDevice(deviceId);
    } catch (e) {
      sessErr = errText(e);
    }
  }

  /**
   * 会话的一行说明：启动时间 + 窗口位置。
   * 位置说明与设备页同源（自动排列 / 让位 / 空间不足降级，tasks 9.6 / 9.8 / 9.9）。
   */
  function sessionMeta(s: SessionInfo): string {
    const time = new Date(s.startedAt).toLocaleTimeString();
    // 「未参与自动排列」的两种原因各用各的措辞（design D5）：平台不支持时说明后果。
    const win = !s.arranged
      ? s.arrangeSkip === 'platform'
        ? '未参与自动排列（窗口位置由系统决定）'
        : '未参与自动排列（已指定窗口位置）'
      : s.slotReused
        ? `窗口 (${s.windowX}, ${s.windowY}) · 空间不足已复用位置`
        : `窗口 (${s.windowX}, ${s.windowY})`;
    return `启动于 ${time} · ${win} · ${s.logLines} 行输出`;
  }
</script>

<div class="sec-head">
  <h2>日志</h2>
  <p>查看每个会话的输出（含已结束的会话）以排查异常，并设置与日志相关的默认参数。</p>
</div>

<div class="grid">
  <div class="card col2">
    <!-- 每个会话的输出都留存下来，用于排查异常退出 -->
    <div class="field">
      <span class="flabel">会话输出</span>
      {#if sessionList.length === 0}
        <div class="ov-empty">
          尚无会话。在设备页点「连接」后，这里会保留每个会话的输出（已结束的也会保留），用于排查异常。
        </div>
      {:else}
        <div class="sess-list">
          {#each sessionList as s (s.deviceId)}
            <div class="sess-item" class:sel={selSession?.deviceId === s.deviceId}>
              <button
                class="sess-pick"
                on:click={() => pickSession(s.deviceId)}
                use:tip={'查看该会话的输出'}
              >
                <span class="status-dot {s.running ? 'connected' : 'idle'}"></span>
                <span class="sess-name">{s.alias}</span>
                <span class="sess-state">
                  {s.running ? `运行中 · PID ${s.pid}` : `已结束 · 退出码 ${s.exitCode ?? '-'}`}
                </span>
              </button>
              {#if s.running}
                <button
                  class="btn ghost sm"
                  on:click={() => stopSession(s.deviceId)}
                >
                  <ZapOff /> 断开
                </button>
              {/if}
            </div>
          {/each}
        </div>

        {#if selSession}
          <div class="sess-cmd">
            <code>{selSession.command}</code>
          </div>
          <div class="sess-meta">{sessionMeta(selSession)}</div>
          <pre class="log-view">{logText || '（该会话暂无输出）'}</pre>
          <div class="log-actions">
            <button
              class="btn ghost sm"
              on:click={refreshLog}
            >
              <RefreshCw /> 刷新
            </button>
            <button
              class="btn ghost sm"
              on:click={clearLog}
              use:tip={'清空该会话已保留的输出（不可恢复）'}
            >
              <Eraser /> 清空
            </button>
            <span class="hint">运行中的会话每 2 秒自动刷新一次。</span>
          </div>
        {/if}
      {/if}
      {#if sessErr}
        <div class="hint dl-error">{sessErr}</div>
      {/if}
      {#if logErr}
        <div class="hint dl-error">{logErr}</div>
      {/if}
    </div>
  </div>

  <!-- 「运行日志参数」排在会话输出之下（design D5）：排查时先看输出，参数是顺手调的第二步 -->
  <div class="card col2">
    <h3>{RUN_LOG_CARD.title}</h3>
    {#each RUN_LOG_CARD.fields as f (f.key)}
      <ParamField
        field={f}
        value={adv[f.key]}
        error={$defaultsErrors[refKey(ADVANCED_GROUP.key, f.key)] ?? null}
        on:change={(e) => updateAdv(f.key, e.detail)}
      />
    {/each}
  </div>

  <!-- 「日志文件」排在**末位**：既有的「会话输出 → 运行日志参数」顺序与其判定逐字不变
       （`parameter-configuration` 的「运行日志参数位于会话输出之后」因此不需要 delta）。
       卡内不展示日志目录路径与当前日志文件名 —— 查看位置由「打开日志目录」承担。 -->
  <div class="card col2">
    <h3>日志文件</h3>
    <!-- 「打开日志目录」与保留策略同处一行：两者是同一件事的两半（这份日志留在哪、留多久），
         而按钮的文字已经说明它去哪 —— 不必再有一个「日志目录」标签把同一件事说第二遍。
         这一行复用既有的 `.path-row`（本仓「一颗按钮 + 就地反馈文字」的横排类），零新样式；
         `.hint` 仍留在行下方：它讲的是保留期与清理口径，是整行共有的说明。 -->
    <div class="field">
      <span class="flabel">保留策略</span>
      <div class="path-row">
        <div class="seg">
          {#each RETENTIONS as r (r.value)}
            <button class:active={retention === r.value} on:click={() => setRetention(r.value)}
              >{r.label}</button
            >
          {/each}
        </div>
        <button class="btn sm" on:click={openLogDirNow}>
          <FolderOpen /> 打开日志目录
        </button>
        <span class="status-text {logDirCls}">{logDirMsg}</span>
      </div>
      <!-- 文案里写的是**命名格式**而不是某个具体日期：写死一个示例日期会在当天恰好被当成
           「当前日志文件名」展示出去，而本卡片不展示当前文件名（只给打开目录这一个入口）。
           必须是 `div`（块级）而不是 `span`：参数卡片里的提示一直用 `div`（见 `ParamField.svelte`），
           这里与它保持同一写法。 -->
      <div class="hint">
        日志按日期命名（<code>yyyy-MM-dd.log</code>）保存在应用数据目录的 logs 文件夹中，
        超出保留期的会按文件名里的日期自动清理；「保留永久」不清理。
      </div>
    </div>
  </div>
</div>
