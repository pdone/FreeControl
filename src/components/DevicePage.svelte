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
    addDevice,
    renameDevice,
    deleteDevice,
    overrides,
    overrideErrors,
    defaults,
    setOverrideValue,
    setOverrideEnabled,
    countOverrideKeys,
    isOverrideEnabled,
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
  } from '../lib/store';
  import { errText } from '../lib/install';
  import { tip } from '../lib/tooltip';
  import { OVERRIDE_FIELD_REFS, fieldOf, refKey, type FieldRef } from '../lib/params';
  import ParamField from './ParamField.svelte';
  import type {
    ConnectResult,
    Device,
    DeviceKind,
    DeviceStatusView,
    OverrideEntry,
    SessionInfo,
  } from '../lib/types';
  import {
    Plus,
    Zap,
    ZapOff,
    Pencil,
    Trash2,
    TabletSmartphone,
    AlertTriangle,
    SlidersHorizontal,
    ScrollText,
    Usb,
    Wifi,
  } from 'lucide-svelte';

  // ── 新增设备（tasks 6.1）──
  let addOpen = false;
  let alias = '';
  let kind: DeviceKind = 'usb';
  let serial = '';
  let ip = '';
  let port = 5555;
  let formErr = '';
  let saving = false;

  // ── 别名内联编辑（tasks 6.2）──
  let editingId: string | null = null;
  let editValue = '';
  let editCancelled = false;
  /** 每台设备各自的错误 / 提示：提示要贴在出错的那一行，不放全局。 */
  let rowErr: Record<string, string> = {};

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
  $: summary = !$deviceStatusMeta.probed
    ? $deviceStatusMeta.error
      ? `设备状态不可用：${$deviceStatusMeta.error}`
      : '正在检测设备状态…'
    : $deviceStatusMeta.stale
      ? '状态待确认（探测失败，沿用上次结果）'
      : '';

  // ── 参数覆盖（tasks 7.4–7.8 / design D5）────────────────────
  //
  // 模型：`overrides[deviceId] = { enabled, values }`，写回 config 后去抖落盘。
  // 开关关闭时取值**不清除**（面板照旧回显），只是不再参与命令拼装。
  //
  // **Svelte 的依赖跟踪只认「模板/响应式表达式里直接出现的变量」**：把 `$store` 藏在
  // 函数体里读，订阅会生效但各块的更新都会被 dirty 检查跳过 —— 表现为「覆盖值改了、
  // 命令预览（另一个组件）也变了，可本页的『已启用设备覆盖』标识与面板输入不跟着动」。
  // 所以这里先把用到的 store 值显式取出来（下面三条 `$:`），模板直接引用它们，
  // 计算则交给接收参数的纯函数。

  /** 覆盖面板是否展开（按设备 id 记住，列表重渲染不会自己收起）。 */
  let openPanels: Record<string, boolean> = {};

  /** 覆盖表：`deviceId → { enabled, values }`。 */
  $: ovById = $overrides;
  /** 各设备覆盖取值的字段错误。 */
  $: ovErrById = $overrideErrors;
  /** 默认参数（8 组），面板未覆盖的项回显用。 */
  $: defaultGroups = $defaults as unknown as Record<string, Record<string, unknown>>;

  function ovValuesOf(
    entry: OverrideEntry | undefined,
  ): Record<string, Record<string, unknown>> {
    return (entry?.values as Record<string, Record<string, unknown>>) ?? {};
  }

  /**
   * 覆盖取值项数：只服务于展开面板标题的「覆盖已禁用（取值已保留）」判定。
   *
   * 判据取「有取值」而非「有条目」—— 拨开关后又关掉、一条值都没填的设备，不该在标题里
   * 报出一个空的禁用提示（用户看不到任何"已保留的取值"）。设备列表那枚两态标识的判据是
   * store 的 `isOverrideActive`（有取值 && 开关启用），此处不再承担标识。
   */
  function ovCount(entry: OverrideEntry | undefined): number {
    return countOverrideKeys(entry?.values as Record<string, unknown>);
  }

  function ovOn(entry: OverrideEntry | undefined): boolean {
    return isOverrideEnabled(entry);
  }

  /** 面板回显：有覆盖值就显示覆盖值（禁用状态也保留可见），否则回落到默认值。 */
  function ovDisplay(
    entry: OverrideEntry | undefined,
    ref: FieldRef,
    defaults: Record<string, Record<string, unknown>>,
  ): unknown {
    const group = ovValuesOf(entry)[ref.group];
    if (group && ref.key in group) return group[ref.key];
    return defaults[ref.group]?.[ref.key];
  }

  /** 覆盖字段的校验提示：只报**该设备真正覆盖**的键，不把默认值的问题记到设备头上。 */
  function ovError(errors: Record<string, string> | undefined, ref: FieldRef): string | null {
    return errors?.[refKey(ref.group, ref.key)] ?? null;
  }

  /**
   * 覆盖面板要渲染的字段（7.8）：直接引用 8 组字段模型里的定义。
   *
   * 这里先解析成列表再 `{#each}`，**不要**在 `{#each}` 里用 `{@const}` + `{#if}` 包裹
   * ParamField —— 那样 Svelte 会把子组件的属性更新挂在 `{@const}` 的 dirty 位上，
   * 而该值恒为常量 ⇒ 覆盖值变化时面板里输入框的数值回显（如 `8 Mbps`）不会跟着刷新。
   */
  $: ovFields = OVERRIDE_FIELD_REFS.map((ref) => ({ ref, field: fieldOf(ref.group, ref.key) })).filter(
    (x): x is { ref: FieldRef; field: NonNullable<ReturnType<typeof fieldOf>> } => !!x.field,
  );

  function togglePanel(id: string): void {
    // 展开参数面板即视为选中该设备：预览要跟着这台设备走（避免「改的是这台、预览的是那台」）。
    selectDevice(id);
    openPanels = { ...openPanels, [id]: !openPanels[id] };
  }

  function onOvChange(id: string, ref: FieldRef, v: unknown): void {
    selectDevice(id);
    setOverrideValue(id, ref.group, ref.key, v);
  }

  function onOvEnable(id: string, e: Event): void {
    setOverrideEnabled(id, (e.target as HTMLInputElement).checked);
  }

  function statusLabel(s: DeviceStatusView): string {
    if (s === 'connected') return '已连接';
    // 状态值 `idle` 在界面上的称谓是「在线」（已探测到、未连接）—— 值本身是后端契约，不改。
    if (s === 'idle') return '在线';
    if (s === 'offline') return '离线';
    return '检测中…';
  }

  /** 连接入口的说明：不可用时把原因写在按钮上，可用时说明这一步会做什么。
      禁用原因取自 store 的 `connectBlockedReason`（与命令条的连接按钮共用同一份措辞）。 */
  function connectHint(d: Device, s: DeviceStatusView): string {
    if (s === 'connected') return '断开该设备的会话';
    const blocked = connectBlockedReason(d, s);
    if (blocked) return blocked;
    if (s === 'offline') return '点「连接」会先尝试重新连上这台设备，成功后再打开投屏';
    return '连接该设备';
  }

  function selectDevice(id: string): void {
    selectedDeviceId.set(id);
  }

  function onCardKeydown(e: KeyboardEvent, id: string): void {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      selectDevice(id);
    }
  }

  /** 会话窗口位置的说明：自动排列的坐标 / 让位 / 平台不支持定位 / 空间不足降级（tasks 9.6 / 9.8 / 9.9 / design D5）。 */
  function windowText(s: SessionInfo): string {
    if (!s.arranged) {
      // 两种「未参与」的原因文案不同：用户指定位置是自己的选择，平台不支持则要明确说明后果。
      return s.arrangeSkip === 'platform'
        ? '未参与自动排列（本会话下窗口位置由系统决定，可能出现重叠）'
        : '未参与自动排列（该设备已指定 --window-x / --window-y）';
    }
    const pos = `窗口 (${s.windowX}, ${s.windowY})`;
    return s.slotReused ? `${pos} · 空间不足已复用位置，可能与其他窗口重叠` : pos;
  }

  /** 批量连接的结果汇总：成功的台数 + 逐台失败原因 + 排列降级提示（tasks 9.2 / 9.8）。 */
  function connectSummary(res: ConnectResult): string {
    const ok = res.outcomes.filter((o) => o.ok).length;
    const failed = res.outcomes.filter((o) => !o.ok);
    const parts = res.outcomes.length > 0 ? [`已连接 ${ok} 台`] : [];
    if (failed.length > 0) {
      parts.push(
        `失败 ${failed.length} 台：` +
          failed.map((f) => `${f.alias || f.deviceId}（${f.error ?? '未知原因'}）`).join('；'),
      );
    }
    if (res.warning) parts.push(res.warning);
    return parts.join(' · ') || '没有可连接的设备';
  }

  /** 连接单台（tasks 9.1）；失败原因贴在对应设备那一行，不放全局。 */
  async function onConnect(d: Device): Promise<void> {
    rowErr = { ...rowErr, [d.id]: '' };
    busy = { ...busy, [d.id]: true };
    sessionNotice.set('');
    try {
      await connectDevice(d.id);
    } catch (e) {
      rowErr = { ...rowErr, [d.id]: errText(e) };
    } finally {
      busy = { ...busy, [d.id]: false };
    }
  }

  /** 断开单台（tasks 9.1）：会话结束、槽位释放，状态回到在线（未连接）。 */
  async function onDisconnect(d: Device): Promise<void> {
    rowErr = { ...rowErr, [d.id]: '' };
    busy = { ...busy, [d.id]: true };
    try {
      await disconnectDevice(d.id);
    } catch (e) {
      rowErr = { ...rowErr, [d.id]: errText(e) };
    } finally {
      busy = { ...busy, [d.id]: false };
    }
  }

  /** 全部连接（tasks 9.2）：只提交在线且未连接的设备，离线设备不会被尝试。 */
  async function onConnectAll(): Promise<void> {
    toolBusy = true;
    rowErr = {};
    sessionNotice.set('');
    try {
      sessionNotice.set(connectSummary(await connectAll()));
    } catch (e) {
      sessionNotice.set(`全部连接失败：${errText(e)}`);
    } finally {
      toolBusy = false;
    }
  }

  /** 全部断开（tasks 9.2）：结束全部在运行的会话。 */
  async function onDisconnectAll(): Promise<void> {
    toolBusy = true;
    rowErr = {};
    sessionNotice.set('');
    const before = runningCount;
    try {
      const list = await disconnectAll();
      const after = list.filter((s) => s.running).length;
      sessionNotice.set(
        before === 0 ? '当前没有正在运行的会话' : `已结束 ${before - after} 个会话`,
      );
    } catch (e) {
      sessionNotice.set(`全部断开失败：${errText(e)}`);
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
    formErr = '';
    alias = '';
    kind = 'usb';
    serial = '';
    ip = '';
    port = 5555;
  }

  function closeAdd(): void {
    addOpen = false;
    formErr = '';
  }

  /** 无线设备必须有 IP —— 缺失时禁用保存，其余校验以后端为准（错误原文直接展示）。 */
  $: addBlocked = kind === 'wifi' && ip.trim() === '';

  async function submitAdd(): Promise<void> {
    formErr = '';
    saving = true;
    try {
      // 别名为空时给个默认名：别名是设备列表里唯一可读的区分依据（别名本身允许中文 / 空格）。
      const device: Device = {
        id: '',
        alias: alias.trim() || `设备${$devices.length + 1}`,
        kind,
        serial: kind === 'usb' ? serial.trim() : '',
        ip: kind === 'wifi' ? ip.trim() : '',
        port: kind === 'wifi' ? Number(port) || 5555 : 0,
      };
      await addDevice(device);
      closeAdd();
    } catch (e) {
      formErr = errText(e);
    } finally {
      saving = false;
    }
  }

  // ── 别名内联编辑 ───────────────────────────────────────────

  function startEdit(d: Device): void {
    rowErr = { ...rowErr, [d.id]: '' };
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
      rowErr = { ...rowErr, [d.id]: '别名不能为空' };
      return;
    }
    try {
      // 改名立即落盘（tasks 6.2）：后端返回落盘后的整份配置，前端整体替换，重启后仍是新别名。
      await renameDevice(d.id, value);
    } catch (e) {
      rowErr = { ...rowErr, [d.id]: errText(e) };
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
      rowErr = { ...rowErr, [d.id]: errText(e) };
      pendingDelete = null;
    } finally {
      delBusy = false;
    }
  }
</script>

<div class="sec-head">
  <h2>设备</h2>
  <p>
    统一管理所有设备：为每台设备设置别名并映射到连接标识（USB 为 serial，WiFi 为 IP:端口），
    查看实时状态，批量或单独连接 / 断开。点选某台设备查看其拼装命令；展开「参数」可针对该设备覆盖默认参数。
  </p>
</div>

<div class="dev-toolbar">
  <button class="btn sm primary" on:click={openAdd}><Plus /> 添加设备</button>
  <span
    class="tip-host"
    use:tip={'连接所有在线且未连接的设备（离线设备不会被尝试连接）'}
  >
    <button
      class="btn sm"
      on:click={onConnectAll}
      disabled={toolBusy || connectableCount === 0}
    >
      <Zap /> 全部连接{toolBusy ? '…' : ''}
    </button>
  </span>
  <span class="tip-host" use:tip={'结束所有正在运行的会话'}>
    <button class="btn sm" on:click={onDisconnectAll} disabled={toolBusy || runningCount === 0}>
      <ZapOff /> 全部断开
    </button>
  </span>
  <div class="spacer"></div>
  {#if summary}
    <span class="status-text" use:tip={$deviceStatusMeta.error || null}>{summary}</span>
  {/if}
  <!-- 批量操作的结果**就地**呈现在本行内（不另起一行）：与设备概况并排，概况在前、结果在后。
       两者语义不同（前者是探测异常的长驻提醒、后者是动作回执），故并排而不互相覆盖。 -->
  {#if $sessionNotice}
    <span class="status-text">{$sessionNotice}</span>
  {/if}
</div>

{#if $devices.length === 0}
  <div class="card col2" style="text-align:center;color:var(--text-faint);padding:40px 20px">
    暂无设备。点击「添加设备」登记你的第一台设备（USB serial 或 WiFi IP:端口）。
  </div>
{:else}
  <div class="dev-list">
    {#each $devices as d (d.id)}
      {@const s = $deviceStatus[d.id] ?? 'unknown'}
      {@const sess = sessionByDevice[d.id]}
      {@const win = sess ? windowText(sess) : ''}
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
                  aria-label="设备别名"
                />
              {:else}
                <span class="a-text">{d.alias}</span>
                <span
                  class="edit"
                  role="button"
                  tabindex="0"
                  aria-label="重命名"
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
                  已启用设备覆盖
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
                {d.connectionId || '（待检测）'}
              </span>
              <span>·</span>
              <span class="status-text {s}">{statusLabel(s)}</span>
              {#if s === 'offline'}
                <span class="status-text offline">
                  {#if d.kind === 'wifi'}
                    可尝试连接
                  {:else}
                    {d.connectionId ? '不可连接' : 'serial 待检测'}
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
                  上次会话已结束（退出码 {sess.exitCode ?? '-'}）
                </span>
              {/if}
              {#if rowErr[d.id]}
                <span class="status-text offline">{rowErr[d.id]}</span>
              {/if}
            </div>
          </div>
          <div class="dev-actions">
            {#if s === 'connected'}
              <span class="tip-host" use:tip={connectHint(d, s)}>
                <button
                  class="btn sm"
                  disabled={busy[d.id]}
                  on:click|stopPropagation={() => onDisconnect(d)}
                >
                  <ZapOff /> {busy[d.id] ? '断开中…' : '断开'}
                </button>
              </span>
            {:else}
              <span class="tip-host" use:tip={connectHint(d, s)}>
                <button
                  class="btn sm primary"
                  disabled={!canConnect(d, s) || busy[d.id]}
                  on:click|stopPropagation={() => onConnect(d)}
                >
                  <Zap /> {busy[d.id] ? '连接中…' : '连接'}
                </button>
              </span>
            {/if}
            {#if sess}
              <button
                class="btn sm ghost"
                on:click|stopPropagation={() => openLog(d.id)}
                use:tip={'在「设置」页查看该会话输出（用于排查问题）'}
              >
                <ScrollText /> 日志
              </button>
            {/if}
            <button
              class="btn sm ghost"
              on:click|stopPropagation={() => togglePanel(d.id)}
              use:tip={'为该设备单独设置参数（覆盖默认参数）'}
            >
              <SlidersHorizontal /> 参数
            </button>
            <button
              class="btn sm danger"
              on:click|stopPropagation={() => askDelete(d)}
              use:tip={'删除该设备（连带清除其参数覆盖与覆盖开关）'}
            >
              <Trash2 /> 删除
            </button>
          </div>
        </div>

        {#if openPanels[d.id]}
          <div class="dev-params">
            <div class="ov-head">
              <span class="lab">
                <SlidersHorizontal /> 为「{d.alias}」单独设置参数{ovCount(ovById[d.id]) > 0 &&
                !ovOn(ovById[d.id])
                  ? ' · 覆盖已禁用（取值已保留）'
                  : ''}
              </span>
              <label
                class="switch"
                use:tip={'启用 / 禁用该设备的参数覆盖（禁用不清除已填写的覆盖值）'}
              >
                <input
                  type="checkbox"
                  checked={ovOn(ovById[d.id])}
                  on:change={(e) => onOvEnable(d.id, e)}
                />
                <span class="slider"></span>
              </label>
            </div>
            <div class="ov-grid" class:ov-off={!ovOn(ovById[d.id])}>
              {#each ovFields as item (item.ref.group + '.' + item.ref.key)}
                <ParamField
                  field={item.field}
                  value={ovDisplay(ovById[d.id], item.ref, defaultGroups)}
                  error={ovError(ovErrById[d.id], item.ref)}
                  on:change={(e) => onOvChange(d.id, item.ref, e.detail)}
                />
              {/each}
              <div class="field">
                <span class="flabel">更多参数</span>
                <div class="ov-empty">
                  完整参数请在「显示源 / 视频 / 音频…」等标签页以默认参数设置；此处覆盖最常用项。
                </div>
              </div>
            </div>
          </div>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<!-- 添加设备（spec「添加 USB 设备 / 添加无线设备」；布局对齐原型 addModal） -->
{#if addOpen}
  <div class="overlay show">
    <div class="modal">
      <div class="ico"><TabletSmartphone /></div>
      <h2>添加设备</h2>
      <p>
        设置别名并选择连接方式。连接标识（serial 或 IP:端口）将作为 scrcpy 的 <code>-s</code> 参数。
      </p>
      <div class="field">
        <label for="dev-alias">别名</label>
        <input id="dev-alias" type="text" bind:value={alias} placeholder="例如：我的主力机" />
      </div>
      <div class="field">
        <span class="flabel">连接方式</span>
        <div class="seg">
          <button class:active={kind === 'usb'} on:click={() => (kind = 'usb')}>USB（serial）</button>
          <button class:active={kind === 'wifi'} on:click={() => (kind = 'wifi')}>无线（IP:端口）</button>
        </div>
      </div>
      <!-- 两种连接方式同处一格（`.field-swap`）：容器高度取两者中的较高者，切换只改可见性
           ⇒ 弹窗高度与位置不随连接方式变化（openspec/config.yaml ⑭ 的「等高收尾」）。
           两支都留在 DOM 里：`bind:value` 照常，`submitAdd` 只按当前 kind 取用对应那一支。 -->
      <div class="field-swap">
        <div class="field" class:off={kind !== 'usb'}>
          <label for="dev-serial">设备 serial</label>
          <input
            id="dev-serial"
            type="text"
            bind:value={serial}
            placeholder="08081FDD4006LYE（可留空，稍后自动检测）"
          />
          <div class="hint">
            serial 可留空：设备会先登记，待检测到唯一一台未登记的 USB 设备后自动补全连接标识。
          </div>
        </div>
        <div class="field" class:off={kind !== 'wifi'}>
          <label for="dev-ip">IP 地址 : 端口</label>
          <div class="row">
            <input id="dev-ip" type="text" bind:value={ip} placeholder="192.168.1.50" />
            <input
              type="number"
              bind:value={port}
              min="1"
              max="65535"
              style="max-width:110px"
              aria-label="端口"
            />
          </div>
          <div class="hint">
            需先在设备上开启无线调试（<code>adb tcpip 5555</code>）；端口留默认 5555 即可。
          </div>
        </div>
      </div>
      {#if formErr}
        <div class="hint dl-error">{formErr}</div>
      {/if}
      <div class="actions">
        <button class="btn ghost" on:click={closeAdd}>取消</button>
        <button class="btn primary" on:click={submitAdd} disabled={saving || addBlocked}>
          保存设备
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- 删除确认（spec「设备记录的增删」：删除前给出确认，避免误删） -->
{#if pendingDelete}
  <div class="overlay show">
    <div class="modal">
      <div class="ico warn"><AlertTriangle /></div>
      <h2>删除设备？</h2>
      <p>
        将删除「{pendingDelete.alias}」（{pendingDelete.connectionId || '连接标识待检测'}）的登记记录，
        并<b>一并清除该设备的参数覆盖与覆盖开关</b>。此操作不可撤销。
      </p>
      <div class="actions">
        <button class="btn ghost" on:click={() => (pendingDelete = null)} disabled={delBusy}>
          取消
        </button>
        <button class="btn danger" on:click={confirmDelete} disabled={delBusy}>
          <Trash2 /> 删除
        </button>
      </div>
    </div>
  </div>
{/if}
