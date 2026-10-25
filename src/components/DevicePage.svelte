<script lang="ts">
  // 设备页（design D11）：设备管理 + 连接管理合并为一张卡片
  // —— 别名 / 连接标识 / 实时状态 / 连接断开 / 删除全部落在同一张卡片上，不拆成两页。
  // 工具栏固定为「添加设备 / 全部连接 / 全部断开」，**不设「全选」**。
  //
  // 实时状态（tasks 6.3 / 6.4）一律取后端下发的 `deviceStatus`：前端不探测 adb、不自己
  // 判定「谁在用这台设备」。页内不回显汇总计数 —— 三项计数只由顶部栏的设备概况指示展示
  // （与这里读同一份数据），页内再重复一遍只是同义反复。
  // 连接入口的可用性由 `canConnect` 一处判定：在线设备直接连，**离线的无线设备**也能连
  // （后端先建立链路，见 specs「无线设备的链路建立」），离线的 USB 设备不行。
  // 状态尚未探测出来时不放行，不留「点了才报错」的路径。
  import {
    devices,
    deviceStatus,
    deviceStatusMeta,
    selectedDeviceId,
    renameDevice,
    deleteDevice,
    overrides,
    isOverrideActive,
    sessions,
    sessionNotice,
    selectedSessionId,
    revealSessionOutput,
    connectDevice,
    disconnectDevice,
    connectAll,
    disconnectAll,
    canConnect,
    connectBlockedReason,
    page,
    type SessionNotice,
  } from '../lib/store';
  import { t } from '../lib/i18n';
  import { errorTextOf, type Translate } from '../lib/message';
  import { tip } from '../lib/tooltip';
  import AddDeviceDialog from './AddDeviceDialog.svelte';
  import DeviceParamsDialog from './DeviceParamsDialog.svelte';
  import { resolveWifiPort } from '../lib/api';
  import type {
    ConnectResult,
    Device,
    DeviceStatusView,
    SessionInfo,
  } from '../lib/types';
  import {
    Plus,
    Zap,
    ZapOff,
    Pencil,
    Trash2,
    AlertTriangle,
    SlidersHorizontal,
    ScrollText,
    Usb,
    Wifi,
    RefreshCw,
  } from 'lucide-svelte';

  // ── 新增设备 ──
  let addOpen = false;

  // ── 别名内联编辑（tasks 6.2）──
  let editingId: string | null = null;
  let editValue = '';
  let editCancelled = false;
  /** 每台设备各自的错误 / 提示：提示要贴在出错的那一行，不放全局。存原始错误以便重译。 */
  let rowErr: Record<string, unknown> = {};

  // ── 删除确认（tasks 6.5）──
  let pendingDelete: Device | null = null;
  let delBusy = false;

  // ── 会话控制（Group 9：tasks 9.1–9.3）──
  //
  // 按钮只驱动 store；「谁在用这台设备」由后端判定后下发（会话表优先于 adb 探测，design D13），
  // 因此按钮的可用性直接读 `deviceStatus`，不在这里另算一套状态。
  /** 工具条按钮在途（批量连接 / 批量断开）。 */
  let toolBusy = false;
  /** 单台设备的连接 / 断开在途。 */
  let busy: Record<string, boolean> = {};

  // 会话（Group 9）：一台设备至多一条记录，**已结束的记录也保留**（其输出供排查，tasks 9.4）。
  // 展开成按设备索引的映射，卡片里按 id 取用 —— 不在模板里对每个字段调函数筛选。
  // 注：`$:` 声明上不能写类型标注，这里的类型由 `Object.fromEntries` 推出（`{ [k: string]: SessionInfo }`）。
  $: sessionList = $sessions;
  $: sessionByDevice = Object.fromEntries(sessionList.map((s) => [s.deviceId, s]));
  $: runningCount = sessionList.filter((s) => s.running).length;

  /** 在线但未连接的设备数（工具条据此禁用「全部连接」，不留点了没反应的按钮）。 */
  $: connectableCount = $devices.filter((d) => $deviceStatus[d.id] === 'idle').length;
  // 工具条右端只承载**设备状态探测自身的异常**：检测中 / 状态不可用 / 状态待确认。
  // 探测正常时为空串、整行不渲染 —— 三项计数由顶部栏的设备概况指示承担，页内不重复。
  $: statusError = $deviceStatusMeta.error
    ? errorTextOf($deviceStatusMeta.error, $t)
    : '';
  $: summary = !$deviceStatusMeta.probed
    ? statusError
      ? $t('device.statusUnavailable', { reason: statusError })
      : $t('device.statusChecking')
    : $deviceStatusMeta.stale
      ? $t('device.statusStale')
      : '';

  $: noticeText = $sessionNotice ? renderNotice($sessionNotice, $t) : '';

  function renderNotice(notice: SessionNotice, tr: Translate): string {
    switch (notice.kind) {
      case 'warning':
        return errorTextOf(notice.warning, tr);
      case 'connectAll':
        return connectSummary(notice.result, tr);
      case 'disconnectAll':
        return notice.before === 0
          ? tr('device.disconnectSummary.none')
          : tr('device.disconnectSummary.done', { count: notice.before - notice.after });
      case 'error':
        return `${tr(notice.labelKey)}${tr('common.detailSeparator')}${errorTextOf(
          notice.error,
          tr,
        )}`;
    }
  }

  // ── 参数覆盖（tasks 7.4–7.8 / design D5）────────────────────
  //
  // 覆盖的编辑面已改为弹窗（spec「设备参数弹窗」，design D2–D8）：设备卡只留「参数」
  // 入口，不再在卡内展开面板 —— 一台设备改参数不会把其它设备推走，列表几何因此与
  // 是否在改参数无关。
  //
  // **Svelte 的依赖跟踪只认「模板/响应式表达式里直接出现的变量」**：把 `$store` 藏在
  // 函数体里读，订阅会生效但各块的更新都会被 dirty 检查跳过 —— 表现为「覆盖值改了、
  // 命令预览（另一个组件）也变了，可本页的『已启用设备覆盖』标识不跟着动」。
  // 所以这里先把用到的 store 值显式取出来，模板直接引用它们。

  /** 正在编辑参数的设备 id（`null` = 弹窗关闭）。 */
  let paramsDeviceId: string | null = null;

  /** 覆盖表：`deviceId → { enabled, values }`。 */
  $: ovById = $overrides;

  function openParams(id: string): void {
    // 打开弹窗**不改**选中设备：这是「查看这台」，不是「切换选中」（design D3）。
    paramsDeviceId = id;
  }

  function statusLabel(s: DeviceStatusView, tr: Translate): string {
    if (s === 'connected') return tr('device.status.connected');
    // 状态值 `idle` 在界面上的称谓是「在线」（已探测到、未连接）—— 值本身是后端契约，不改。
    if (s === 'idle') return tr('device.status.online');
    if (s === 'offline') return tr('device.status.offline');
    return tr('device.status.unknown');
  }

  /** 连接入口的说明：不可用时把原因写在按钮上，可用时说明这一步会做什么。
      禁用原因取自 store 的 `connectBlockedReason`（与命令条的连接按钮共用同一份措辞）。 */
  function connectHint(d: Device, s: DeviceStatusView, tr: Translate): string {
    if (s === 'connected') return tr('device.connectHint.connected');
    const blocked = connectBlockedReason(d, s);
    if (blocked) return tr(blocked);
    if (s === 'offline') return tr('device.connectHint.offlineWifi');
    return tr('device.connectHint.connect');
  }

  function selectDevice(id: string): void {
    selectedDeviceId.set(id);
  }

  /**
   * 记录里的 IP 是否是可解析的 IPv4。
   *
   * 「重新解析端口」靠多播**按 IP** 找服务，而 IP 这一栏也允许填主机名
   * （`adb connect mi10:5555` 是合法的旧法记录）—— 那种记录永远解析不出来，
   * 按钮露出来只会给一句注定失败的报错，故直接不露。
   */
  function isIpv4(raw: string): boolean {
    const parts = raw.trim().split('.');
    if (parts.length !== 4) return false;
    return parts.every((p) => /^\d{1,3}$/.test(p) && Number(p) <= 255);
  }

  function onCardKeydown(e: KeyboardEvent, id: string): void {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      selectDevice(id);
    }
  }

  /** 会话窗口位置的说明：自动排列的坐标 / 让位 / 平台不支持定位 / 空间不足降级（tasks 9.6 / 9.8 / 9.9 / design D5）。 */
  function windowText(s: SessionInfo, tr: Translate): string {
    if (!s.arranged) {
      // 两种「未参与」的原因文案不同：用户指定位置是自己的选择，平台不支持则要明确说明后果。
      return tr(
        s.arrangeSkip === 'platform'
          ? 'device.window.noArrangePlatform'
          : 'device.window.noArrangeUser',
      );
    }
    const pos = tr('device.window.position', { x: s.windowX ?? '', y: s.windowY ?? '' });
    return s.slotReused
      ? tr('device.window.reused', { x: s.windowX ?? '', y: s.windowY ?? '' })
      : pos;
  }

  /** 批量连接的结果汇总：成功的台数 + 逐台失败原因 + 排列降级提示（tasks 9.2 / 9.8）。 */
  function connectSummary(res: ConnectResult, tr: Translate): string {
    const ok = res.outcomes.filter((o) => o.ok).length;
    const failed = res.outcomes.filter((o) => !o.ok);
    const parts =
      res.outcomes.length > 0 ? [tr('device.connectSummary.success', { count: ok })] : [];
    if (failed.length > 0) {
      const details = failed
        .map((f) =>
          tr('device.connectSummary.item', {
            alias: f.alias || f.deviceId,
            reason: f.error ? errorTextOf(f.error, tr) : tr('device.unknownReason'),
          }),
        )
        .join(tr('common.listSeparator'));
      parts.push(tr('device.connectSummary.failed', { count: failed.length, details }));
    }
    if (res.warning) parts.push(errorTextOf(res.warning, tr));
    return parts.join(' · ') || tr('device.connectSummary.none');
  }

  /** 连接单台（tasks 9.1）；失败原因贴在对应设备那一行，不放全局。 */
  async function onConnect(d: Device): Promise<void> {
    rowErr = { ...rowErr, [d.id]: null };
    busy = { ...busy, [d.id]: true };
    sessionNotice.set(null);
    try {
      await connectDevice(d.id);
    } catch (e) {
      rowErr = { ...rowErr, [d.id]: e };
    } finally {
      busy = { ...busy, [d.id]: false };
    }
  }

  /** 断开单台（tasks 9.1）：会话结束、槽位释放，状态回到在线（未连接）。 */
  async function onDisconnect(d: Device): Promise<void> {
    rowErr = { ...rowErr, [d.id]: null };
    busy = { ...busy, [d.id]: true };
    try {
      await disconnectDevice(d.id);
    } catch (e) {
      rowErr = { ...rowErr, [d.id]: e };
    } finally {
      busy = { ...busy, [d.id]: false };
    }
  }

  /** 全部连接（tasks 9.2）：只提交在线且未连接的设备，离线设备不会被尝试。 */
  async function onConnectAll(): Promise<void> {
    toolBusy = true;
    rowErr = {};
    sessionNotice.set(null);
    try {
      const result = await connectAll();
      sessionNotice.set({ kind: 'connectAll', result });
    } catch (e) {
      sessionNotice.set({ kind: 'error', labelKey: 'device.connectAllFailed', error: e });
    } finally {
      toolBusy = false;
    }
  }

  /** 全部断开（tasks 9.2）：结束全部在运行的会话。 */
  async function onDisconnectAll(): Promise<void> {
    toolBusy = true;
    rowErr = {};
    sessionNotice.set(null);
    const before = runningCount;
    try {
      const list = await disconnectAll();
      const after = list.filter((s) => s.running).length;
      sessionNotice.set({ kind: 'disconnectAll', before, after });
    } catch (e) {
      sessionNotice.set({ kind: 'error', labelKey: 'device.disconnectAllFailed', error: e });
    } finally {
      toolBusy = false;
    }
  }

  /**
   * 跳到设置页查看该会话输出（tasks 9.4 的界面入口），并让设置页滚动到页面底部的会话输出卡。
   *
   * 选择「哪条会话」放在 store 里，设备页与设置页的「会话输出」读同一份 —— 不靠路由参数传递。
   */
  function openLog(deviceId: string): void {
    selectedSessionId.set(deviceId);
    page.set('settings');
    revealSessionOutput.set(true);
  }

  // ── 新增设备 ───────────────────────────────────────────────

  function openAdd(): void {
    addOpen = true;
  }

  // ── 别名内联编辑 ───────────────────────────────────────────

  function startEdit(d: Device): void {
    rowErr = { ...rowErr, [d.id]: null };
    editingId = d.id;
    editValue = d.alias;
    editCancelled = false;
  }

  function onEditKeydown(e: KeyboardEvent): void {
    if (e.key === 'Enter') {
      (e.target as HTMLInputElement).blur();
    } else if (e.key === 'Escape') {
      // 取消：先立标记再失焦，避免 blur 把半截输入提交上去。
      editCancelled = true;
      (e.target as HTMLInputElement).blur();
    }
  }

  async function commitEdit(d: Device): Promise<void> {
    const value = editValue.trim();
    const cancelled = editCancelled;
    editingId = null;
    editCancelled = false;
    if (cancelled || value === d.alias) return;
    if (!value) {
      rowErr = { ...rowErr, [d.id]: { code: 'device.aliasEmpty' } };
      return;
    }
    try {
      // 改名立即落盘（tasks 6.2）：后端返回落盘后的整份配置，前端整体替换，重启后仍是新别名。
      await renameDevice(d.id, value);
    } catch (e) {
      rowErr = { ...rowErr, [d.id]: e };
    }
  }

  // ── 删除 ───────────────────────────────────────────────────

  function askDelete(d: Device): void {
    pendingDelete = d;
  }

  async function confirmDelete(): Promise<void> {
    const d = pendingDelete;
    if (!d) return;
    delBusy = true;
    try {
      // 别名映射、参数覆盖与覆盖开关由后端一并清除（tasks 6.5）。
      await deleteDevice(d.id);
      pendingDelete = null;
    } catch (e) {
      rowErr = { ...rowErr, [d.id]: e };
      pendingDelete = null;
    } finally {
      delBusy = false;
    }
  }

  // ── 无线连接（spec `device-management`：配对 / 局域网发现 / 端口重解析）──

  /**
   * 重新解析某台无线设备的连接端口。
   *
   * 只在卡片处于离线时才露出入口 —— 那正是「端口随设备侧重启 / 重开无线调试而变」的表现。
   */
  async function onResolvePort(d: Device): Promise<void> {
    rowErr = { ...rowErr, [d.id]: null };
    busy = { ...busy, [d.id]: true };
    try {
      await resolveWifiPort(d.id);
    } catch (e) {
      rowErr = { ...rowErr, [d.id]: e };
    } finally {
      busy = { ...busy, [d.id]: false };
    }
  }
</script>

<div class="sec-head">
  <h2>{$t('nav.devices')}</h2>
  <p>{$t('device.subtitle')}</p>
</div>

<div class="dev-toolbar">
  <button class="btn sm primary" on:click={openAdd}><Plus /> {$t('device.add')}</button>
  <span class="tip-host" use:tip={$t('device.connectAllTip')}>
    <button
      class="btn sm"
      on:click={onConnectAll}
      disabled={toolBusy || connectableCount === 0}
    >
      <Zap /> {$t('device.connectAll')}{toolBusy ? '…' : ''}
    </button>
  </span>
  <span class="tip-host" use:tip={$t('device.disconnectAllTip')}>
    <button class="btn sm" on:click={onDisconnectAll} disabled={toolBusy || runningCount === 0}>
      <ZapOff /> {$t('device.disconnectAll')}
    </button>
  </span>
  <div class="spacer"></div>
  {#if summary}
    <span class="status-text" use:tip={statusError || null}>{summary}</span>
  {/if}
  <!-- 批量操作的结果**就地**呈现在本行内（不另起一行）：与设备概况并排，概况在前、结果在后。
       两者语义不同（前者是探测异常的长驻提醒、后者是动作回执），故并排而不互相覆盖。 -->
  {#if $sessionNotice}
    <span class="status-text">{noticeText}</span>
  {/if}
</div>

{#if $devices.length === 0}
  <div class="card col2" style="text-align:center;color:var(--text-faint);padding:40px 20px">
    {$t('device.empty')}
  </div>
{:else}
  <div class="dev-list">
    {#each $devices as d (d.id)}
      {@const s = $deviceStatus[d.id] ?? 'unknown'}
      {@const sess = sessionByDevice[d.id]}
      {@const win = sess ? windowText(sess, $t) : ''}
      <div
        class="dev-card"
        class:sel={$selectedDeviceId === d.id}
        class:connected={s === 'connected'}
        class:offline={s === 'offline'}
      >
        <div
          class="dev-main"
          role="button"
          tabindex="0"
          on:click={() => selectDevice(d.id)}
          on:keydown={(e) => onCardKeydown(e, d.id)}
        >
          <span class="status-dot {s}"></span>
          <div class="dev-id">
            <div class="dev-alias">
              {#if editingId === d.id}
                <input
                  type="text"
                  bind:value={editValue}
                  on:blur={() => commitEdit(d)}
                  on:keydown={onEditKeydown}
                  aria-label={$t('device.aliasAria')}
                />
              {:else}
                <span class="a-text">{d.alias}</span>
                <span
                  class="edit"
                  role="button"
                  tabindex="0"
                  aria-label={$t('device.renameAria')}
                  on:click|stopPropagation={() => startEdit(d)}
                  on:keydown={(e) => e.key === 'Enter' && startEdit(d)}
                >
                  <Pencil />
                </span>
              {/if}
              <!-- 两态标识：只在"有覆盖取值且开关启用"时出现，MUST NOT 带项数
                   （判据在 store 的 `isOverrideActive`）。禁用态与"一项取值都没填"
                   都不在此表达 —— 前者只在展开面板的标题里说。 -->
              {#if isOverrideActive(ovById[d.id])}
                <span class="tag over">
                  {$t('device.overrideActive')}
                </span>
              {/if}
            </div>
            <div class="dev-meta">
              <!-- 连接方式的标识带图标（与原型同稿）：两种连接方式不读文字也能区分。
                   两枚标识同取强调色族（见 app.css 的 .tag.usb / .tag.wifi）——
                   类别标识不各自占一个色相，颜色留给状态与警示。 -->
              <span class="tag {d.kind}">
                {#if d.kind === 'usb'}<Usb />{:else}<Wifi />{/if}
                {d.kind === 'usb' ? 'USB' : 'WiFi'}
              </span>
              <!-- 连接标识是技术值，要保持可复制（scrcpy 的 -s 取值）：
                   用语义类名进白名单，而不是靠 `:nth-child` 这类结构选择器指它 ——
                   后者在 .dev-meta 增删任一 span 时会静默指错元素。 -->
              <span class="dev-cid">
                {d.connectionId || $t('device.connectionPending')}
              </span>
              <span>·</span>
              <span class="status-text {s}">{statusLabel(s, $t)}</span>
              {#if s === 'offline'}
                <span class="status-text offline">
                  {#if d.kind === 'wifi'}
                    {$t('device.offlineTryConnect')}
                  {:else}
                    {d.connectionId
                      ? $t('device.offlineNotConnectable')
                      : $t('device.offlineSerialPending')}
                  {/if}
                </span>
              {/if}
              {#if sess && s === 'connected'}
                <!-- 会话的真实位置：自动排列给出的坐标、让位说明，或空间不足的降级提示 -->
                <span>·</span>
                <span class="sess-hint">PID {sess.pid} · {win}</span>
              {:else if sess}
                <span>·</span>
                <span class="sess-hint">
                  {$t('device.sessionEnded', { code: sess.exitCode ?? '-' })}
                </span>
              {/if}
              {#if rowErr[d.id]}
                <span class="status-text offline">{errorTextOf(rowErr[d.id], $t)}</span>
              {/if}
            </div>
          </div>
          <div class="dev-actions">
            {#if s === 'connected'}
              <span class="tip-host" use:tip={connectHint(d, s, $t)}>
                <button
                  class="btn sm"
                  disabled={busy[d.id]}
                  on:click|stopPropagation={() => onDisconnect(d)}
                >
                  <ZapOff /> {busy[d.id] ? $t('device.disconnecting') : $t('device.disconnect')}
                </button>
              </span>
            {:else}
              <span class="tip-host" use:tip={connectHint(d, s, $t)}>
                <button
                  class="btn sm primary"
                  disabled={!canConnect(d, s) || busy[d.id]}
                  on:click|stopPropagation={() => onConnect(d)}
                >
                  <Zap /> {busy[d.id] ? $t('device.connecting') : $t('device.connect')}
                </button>
              </span>
            {/if}
            {#if sess}
              <button
                class="btn sm ghost"
                on:click|stopPropagation={() => openLog(d.id)}
                use:tip={$t('device.logTip')}
              >
                <ScrollText /> {$t('device.log')}
              </button>
            {/if}
            <!-- 无线设备的端口会随设备侧重启 / 重开无线调试而变，表现为「离线」。
                 入口只在离线、且记录里填的是 IPv4 时露出（见 `isIpv4`）——
                 避免给正常在线的设备多加一颗按钮，也避免对主机名记录承诺做不到的事。 -->
            {#if s === 'offline' && d.kind === 'wifi' && isIpv4(d.ip)}
              <span class="tip-host" use:tip={$t('wireless.resolveTip')}>
                <button
                  class="btn sm ghost"
                  disabled={busy[d.id]}
                  on:click|stopPropagation={() => onResolvePort(d)}
                >
                  <RefreshCw />
                  {busy[d.id] ? $t('wireless.resolveBusy') : $t('wireless.resolve')}
                </button>
              </span>
            {/if}
            <button
              class="btn sm ghost"
              on:click|stopPropagation={() => openParams(d.id)}
              use:tip={$t('device.paramsTip')}
            >
              <SlidersHorizontal /> {$t('device.params')}
            </button>
            <button
              class="btn sm danger"
              on:click|stopPropagation={() => askDelete(d)}
              use:tip={$t('device.deleteTip')}
            >
              <Trash2 /> {$t('device.delete')}
            </button>
          </div>
        </div>
      </div>
    {/each}
  </div>
{/if}

<!-- 删除确认（spec「设备记录的增删」：删除前给出确认，避免误删） -->
{#if pendingDelete}
  <div class="overlay show">
    <div class="modal">
      <div class="ico warn"><AlertTriangle /></div>
      <h2>{$t('device.delete.title')}</h2>
      <p>
        {$t('device.delete.before', {
          alias: pendingDelete.alias,
          cid: pendingDelete.connectionId || $t('device.delete.connectionPending'),
        })}<b>{$t('device.delete.bold')}</b>{$t('device.delete.after')}
      </p>
      <div class="actions">
        <button class="btn ghost" on:click={() => (pendingDelete = null)} disabled={delBusy}>
          {$t('device.delete.cancel')}
        </button>
        <button class="btn danger" on:click={confirmDelete} disabled={delBusy}>
          <Trash2 /> {$t('device.delete.confirm')}
        </button>
      </div>
    </div>
  </div>
{/if}

<AddDeviceDialog open={addOpen} on:close={() => (addOpen = false)} />

<!-- 参数弹窗用 `{#if}` 挂载：组件卸载即丢弃「扩大」状态，每次打开都回到常态（design D4）。 -->
{#if paramsDeviceId}
  <DeviceParamsDialog deviceId={paramsDeviceId} on:close={() => (paramsDeviceId = null)} />
{/if}
