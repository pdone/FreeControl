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
    config,
    devices,
    sortedDevices,
    sortMode,
    sortDirections,
    setSortMode,
    setSortDirection,
    reorderDevices,
    deviceStatus,
    deviceStatusMeta,
    deviceNotices,
    selectedDeviceId,
    renameDevice,
    setWifiPort,
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
    setDeviceListDensity,
    type SessionNotice,
  } from '../lib/store';
  import { t } from '../lib/i18n';
  import { errorTextOf, type Translate } from '../lib/message';
  import { tip } from '../lib/tooltip';
  import { fade } from 'svelte/transition';
  import { DIALOG_FADE } from '../lib/motion';
  import AddDeviceDialog from './AddDeviceDialog.svelte';
  import DeviceParamsDialog from './DeviceParamsDialog.svelte';
  import DeviceSortSelect from './DeviceSortSelect.svelte';
  import DeviceIssueBadge from './DeviceIssueBadge.svelte';
  import { moveToIndex, sameOrder, flippedDirection } from '../lib/deviceSort';
  import { segSlide } from '../lib/segSlide';
  import type {
    ConnectResult,
    Device,
    DeviceListDensity,
    DeviceStatusView,
    SortDirection,
    SortMode,
  } from '../lib/types';
  import { onDestroy, onMount, tick } from 'svelte';
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
    GripVertical,
    LayoutList,
    StretchHorizontal,
  } from 'lucide-svelte';

  // ── 列表密度（spec「设备列表显示密度」）──
  // 密度属于外观偏好，不进入设备域快照；工具栏与卡片都只读它。
  $: density = ($config?.ui.deviceListDensity ?? 'detailed') as DeviceListDensity;
  $: compactView = density === 'compact';

  function setDensity(next: DeviceListDensity): void {
    if (next === density || densityLocked) return;
    setDeviceListDensity(next);
  }

  // ── 最后连接时间（相对显示）──
  // 每 30 秒心跳只为让「x 分钟前」跟着走；`now` 必须直接出现在模板表达式里，
  // Svelte 4 才会在心跳时重算（把依赖藏进函数体就会静默失更新）。
  let now = Date.now();
  let agoTimer: ReturnType<typeof setInterval> | null = null;
  onMount(() => {
    agoTimer = setInterval(() => (now = Date.now()), 30_000);
  });
  onDestroy(() => {
    if (agoTimer) clearInterval(agoTimer);
  });

  // ── 行内问题指示（spec「行内问题指示与行高稳定」）──
  // 动作失败与输入校验分开：前者只留短标签 / 图标，详情进提示层；后者继续内联可见。
  type RowActionFailure = {
    kind: 'connect' | 'disconnect' | 'delete';
    error: unknown;
  };
  let rowActionErr: Record<string, RowActionFailure | null> = {};

  type DeviceIssue = { label: string; detail: string; tone: 'danger' | 'warn' };
  $: issuesByDevice = Object.fromEntries(
    $devices.map((d): [string, DeviceIssue[]] => {
      const list: DeviceIssue[] = [];
      const fail = rowActionErr[d.id];
      if (fail) {
        list.push({
          label: $t(
            fail.kind === 'connect'
              ? 'device.issue.connectFailed'
              : fail.kind === 'disconnect'
                ? 'device.issue.disconnectFailed'
                : 'device.issue.deleteFailed',
          ),
          detail: errorTextOf(fail.error, $t),
          tone: 'danger',
        });
      }
      const notice = $deviceNotices[d.id];
      if (notice) {
        list.push({
          label: $t('device.issue.notice'),
          detail: errorTextOf(notice, $t),
          tone: 'warn',
        });
      }
      return [d.id, list];
    }),
  ) as Record<string, DeviceIssue[]>;

  // ── 新增设备 ──
  let addOpen = false;

  // ── 别名内联编辑（tasks 6.2）──
  let editingId: string | null = null;
  let editValue = '';
  let editCancelled = false;
  /** 每台设备各自的输入校验错误（别名 / 端口）：短文本直接贴在出错的那一行。存原始错误以便重译。 */
  let rowErr: Record<string, unknown> = {};

  // ── 手动修改无线端口（spec `device-management`「手动修改无线连接端口」）──
  // 与别名同一套内联编辑：只有一台设备在编辑，错误复用 `rowErr` 贴回该行。
  let portEditId: string | null = null;
  let portValue = 0;
  let portCancelled = false;

  /** 输入框挂载即聚焦并全选：失焦提交/关闭因此有可靠前提（见 design D4）。 */
  function focusAndSelect(node: HTMLInputElement): void {
    node.focus();
    node.select();
  }

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

  // ── 排序与重排（spec device-management「设备列表排序方式」等）──
  //
  // `devices` 保持登记顺序（批量连接 / 窗口槽位按它走）；本页只渲染派生顺序。
  // 拖动 / 键盘重排 / 别名编辑期间冻结一份可见顺序，避免状态推送把目标挪走。
  let sortBusy = false;
  let sortError = '';
  let sortAnnouncement = '';
  /** 编辑别名期间冻结的可见顺序（防止输入框因自动重排跳位）。 */
  let frozenIds: string[] | null = null;

  type OrderDraft = {
    id: string;
    ids: string[];
    startIds: string[];
    memberSig: string;
  };
  type DragState = OrderDraft & {
    pointerId: number;
    startX: number;
    startY: number;
    active: boolean;
    dropBefore: string | null;
  };

  let drag: DragState | null = null;
  let keyDrag: OrderDraft | null = null;
  /** 每台设备的手柄：键盘确认后把焦点送回被移动的那一个。 */
  let handleEls: Record<string, HTMLButtonElement> = {};

  onDestroy(() => detachPointer());

  $: sortOptions = [
    { value: 'manual', label: $t('device.sort.manual') },
    {
      value: 'status',
      label: $t('device.sort.status'),
      direction: $sortDirections.status,
      directionLabel: directionText($sortDirections.status),
    },
    {
      value: 'name',
      label: $t('device.sort.name'),
      direction: $sortDirections.name,
      directionLabel: directionText($sortDirections.name),
    },
    {
      value: 'lastConnected',
      label: $t('device.sort.lastConnected'),
      direction: $sortDirections.lastConnected,
      directionLabel: directionText($sortDirections.lastConnected),
    },
  ] as {
    value: SortMode;
    label: string;
    direction?: SortDirection;
    directionLabel?: string;
  }[];
  $: sortedIds = $sortedDevices.map((d) => d.id);
  $: byId = Object.fromEntries($devices.map((d) => [d.id, d]));
  // 编辑期间冻结既有顺序，但新增设备仍要出现（追加在末尾），删除的设备由 byId 过滤掉。
  $: frozenView = frozenIds
    ? [...frozenIds, ...sortedIds.filter((id) => !frozenIds!.includes(id))]
    : null;
  $: activeDraft = drag?.ids ?? keyDrag?.ids ?? frozenView;
  $: visibleIds = activeDraft ?? sortedIds;
  $: visibleDevices = visibleIds
    .map((id) => byId[id])
    .filter((d): d is Device => !!d);
  $: memberSig = $devices.map((d) => d.id).join('|');
  $: sortLocked = !!drag || !!keyDrag || sortBusy;
  /** 重排或内联编辑期间不切换密度：拖拽中改卡高会让落点计算跳动，编辑中输入框可能被移走。 */
  $: densityLocked = sortLocked || editingId !== null || portEditId !== null;

  // 拖动 / 键盘拿起期间设备成员变化：取消本次调整并提示（不提交旧集合）。
  //
  // 取消放到微任务里执行：在同一个响应式块内同步重置 `drag` 会让「显示顺序」那几条
  // 声明与它自己形成写回环，Svelte 可能不再重算 `visibleIds`，列表会停在空数组上。
  $: if ((drag || keyDrag) && (drag ?? keyDrag)!.memberSig !== memberSig) {
    const changedSig = memberSig;
    queueMicrotask(() => {
      const current = drag ?? keyDrag;
      if (current && current.memberSig !== changedSig) {
        cancelReorder('device.sort.listChanged');
      }
    });
  }

  function newDraft(id: string): OrderDraft {
    const ids = visibleIds.slice();
    return { id, ids, startIds: ids, memberSig };
  }

  function announce(text: string): void {
    sortAnnouncement = text;
  }

  function positionText(id: string, ids: string[]): string {
    return $t('device.sort.moved', {
      alias: byId[id]?.alias ?? id,
      index: ids.indexOf(id) + 1,
      total: ids.length,
    });
  }

  function cancelReorder(noticeKey?: Parameters<Translate>[0]): void {
    detachPointer();
    drag = null;
    keyDrag = null;
    if (noticeKey) announce($t(noticeKey));
  }

  async function commitOrder(
    ids: string[],
    droppedId: string,
    restoreFocus = false,
  ): Promise<void> {
    sortBusy = true;
    sortError = '';
    try {
      await reorderDevices(ids);
      announce(
        $t('device.sort.dropped', {
          alias: byId[droppedId]?.alias ?? droppedId,
          index: ids.indexOf(droppedId) + 1,
          total: ids.length,
        }),
      );
    } catch (e) {
      sortError = errorTextOf(e, $t);
    } finally {
      sortBusy = false;
      drag = null;
      keyDrag = null;
      detachPointer();
      if (restoreFocus) {
        // 键盘重排后焦点留在同一台设备的手柄上（DOM 重排 / 重渲染可能让它掉到 body）。
        await tick();
        handleEls[droppedId]?.focus();
      }
    }
  }

  async function onSortModeChange(e: CustomEvent<SortMode>): Promise<void> {
    const mode = e.detail;
    if (mode === $sortMode || sortLocked) return;
    sortBusy = true;
    sortError = '';
    try {
      await setSortMode(mode);
    } catch (err) {
      sortError = `${$t('device.sort.failed')}${$t('common.detailSeparator')}${errorTextOf(err, $t)}`;
    } finally {
      sortBusy = false;
    }
  }

  function directionText(direction: SortDirection): string {
    return $t(
      direction === 'asc' ? 'device.sort.ascending' : 'device.sort.descending',
    );
  }

  /** 再次选择当前自动方式：翻转该方式自己的方向，并播报结果。 */
  async function onSortDirectionToggle(e: CustomEvent<SortMode>): Promise<void> {
    const mode = e.detail;
    if (mode === 'manual' || sortLocked) return;
    const next = flippedDirection($sortDirections[mode]);
    sortBusy = true;
    sortError = '';
    try {
      await setSortDirection(mode, next);
      const label = sortOptions.find((o) => o.value === mode)?.label ?? mode;
      announce(
        $t('device.sort.directionChanged', {
          mode: label,
          direction: directionText(next),
        }),
      );
    } catch (err) {
      sortError = `${$t('device.sort.failed')}${$t('common.detailSeparator')}${errorTextOf(err, $t)}`;
    } finally {
      sortBusy = false;
    }
  }

  // ── 指针拖动 ──
  let listEl: HTMLDivElement;
  let scrollEl: HTMLElement | null = null;
  let lastPointerY = 0;
  let scrollDir = 0;
  let scrollFrame = 0;
  /** 被拖卡片与跟随指针的浮层（浮层是卡片的一份快照，不参与布局与命中测试）。 */
  let dragCardEl: HTMLElement | null = null;
  let ghostEl: HTMLElement | null = null;
  let ghostOffsetX = 0;
  let ghostOffsetY = 0;

  function onHandlePointerDown(e: PointerEvent, d: Device): void {
    if (e.button !== 0 || sortLocked || editingId !== null || visibleDevices.length < 2) return;
    e.preventDefault();
    e.stopPropagation();
    try {
      (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    } catch {
      // 合成事件没有活动指针；真实指针操作不会走到这里。
    }
    dragCardEl = (e.currentTarget as HTMLElement).closest<HTMLElement>('.dev-card');
    const draft = newDraft(d.id);
    drag = {
      ...draft,
      pointerId: e.pointerId,
      startX: e.clientX,
      startY: e.clientY,
      active: false,
      dropBefore: null,
    };
    scrollEl = findScrollParent(listEl);
    window.addEventListener('pointermove', onPointerMove);
    window.addEventListener('pointerup', onPointerUp);
    window.addEventListener('pointercancel', onPointerCancel);
    window.addEventListener('keydown', onDragKeydown, true);
    window.addEventListener('blur', onWindowBlur);
  }

  function detachPointer(): void {
    window.removeEventListener('pointermove', onPointerMove);
    window.removeEventListener('pointerup', onPointerUp);
    window.removeEventListener('pointercancel', onPointerCancel);
    window.removeEventListener('keydown', onDragKeydown, true);
    window.removeEventListener('blur', onWindowBlur);
    removeGhost();
    dragCardEl = null;
    stopAutoScroll();
  }

  /** 越过激活阈值后给被拖卡片做一份浮层，跟手显示当前正在移动的是哪一台。 */
  function startGhost(clientX: number, clientY: number): void {
    const card =
      dragCardEl ??
      listEl?.querySelector<HTMLElement>(
        `[data-device-id="${CSS.escape(drag?.id ?? '')}"]`,
      ) ??
      null;
    if (!card) return;
    const r = card.getBoundingClientRect();
    const clone = card.cloneNode(true) as HTMLElement;
    clone.classList.remove('dragging', 'drop-before', 'sel');
    clone.classList.add('dev-drag-ghost');
    clone.removeAttribute('data-device-id');
    clone.setAttribute('aria-hidden', 'true');
    // 克隆只用于看：不进入 Tab 序列，也不响应指针（点穿到下面的落点计算）。
    clone.setAttribute('inert', '');
    clone.style.width = `${r.width}px`;
    document.body.appendChild(clone);
    ghostEl = clone;
    ghostOffsetX = clientX - r.left;
    ghostOffsetY = clientY - r.top;
    moveGhost(clientX, clientY);
  }

  function moveGhost(clientX: number, clientY: number): void {
    if (!ghostEl) return;
    ghostEl.style.transform = `translate3d(${clientX - ghostOffsetX}px, ${
      clientY - ghostOffsetY
    }px, 0)`;
  }

  function removeGhost(): void {
    ghostEl?.remove();
    ghostEl = null;
  }

  function findScrollParent(el: HTMLElement | null): HTMLElement | null {
    let p = el?.parentElement ?? null;
    while (p) {
      const oy = getComputedStyle(p).overflowY;
      if (oy === 'auto' || oy === 'scroll') return p;
      p = p.parentElement;
    }
    return null;
  }

  function onPointerMove(e: PointerEvent): void {
    const draft = drag;
    if (!draft || e.pointerId !== draft.pointerId) return;
    lastPointerY = e.clientY;
    if (!draft.active) {
      const dx = e.clientX - draft.startX;
      const dy = e.clientY - draft.startY;
      if (Math.hypot(dx, dy) < 5) return;
      drag = { ...draft, active: true };
      scrollEl = findScrollParent(listEl);
      startGhost(e.clientX, e.clientY);
    }
    updateDropTarget(e.clientY);
    moveGhost(e.clientX, e.clientY);
    updateAutoScroll(e.clientY);
  }

  /** 按指针相对卡片中点的位置计算插入点（序列不含被拖设备）。 */
  function updateDropTarget(pointerY: number): void {
    const draft = drag;
    if (!draft || !listEl) return;
    const cards = Array.from(listEl.querySelectorAll<HTMLElement>('[data-device-id]')).filter(
      (el) => el.dataset.deviceId !== draft.id,
    );
    let index = cards.length;
    for (let i = 0; i < cards.length; i++) {
      const r = cards[i].getBoundingClientRect();
      if (pointerY < r.top + r.height / 2) {
        index = i;
        break;
      }
    }
    const ids = moveToIndex(draft.startIds, draft.id, index);
    drag = { ...draft, ids, dropBefore: cards[index]?.dataset.deviceId ?? null };
  }

  function updateAutoScroll(pointerY: number): void {
    if (!scrollEl) return;
    const r = scrollEl.getBoundingClientRect();
    const edge = 48;
    const speed = 12;
    scrollDir = pointerY < r.top + edge ? -speed : pointerY > r.bottom - edge ? speed : 0;
    if (scrollDir && !scrollFrame) {
      const step = (): void => {
        if (!drag?.active || !scrollEl || !scrollDir) {
          scrollFrame = 0;
          return;
        }
        scrollEl.scrollTop += scrollDir;
        updateDropTarget(lastPointerY);
        scrollFrame = requestAnimationFrame(step);
      };
      scrollFrame = requestAnimationFrame(step);
    }
  }

  function stopAutoScroll(): void {
    scrollDir = 0;
    if (scrollFrame) cancelAnimationFrame(scrollFrame);
    scrollFrame = 0;
  }

  function onPointerUp(e: PointerEvent): void {
    const draft = drag;
    if (!draft || e.pointerId !== draft.pointerId) return;
    const moved = draft.active && !sameOrder(draft.ids, draft.startIds);
    detachPointer();
    if (moved) void commitOrder(draft.ids, draft.id);
    else {
      drag = null;
      keyDrag = null;
    }
  }

  function onPointerCancel(): void {
    cancelReorder();
  }

  function onWindowBlur(): void {
    if (sortBusy) return;
    if (drag || keyDrag) cancelReorder();
  }

  function onDragKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape' && !sortBusy && (drag || keyDrag)) {
      e.preventDefault();
      cancelReorder('device.sort.cancelled');
    }
  }

  // ── 键盘重排 ──

  function onHandleKeydown(e: KeyboardEvent, d: Device): void {
    if (sortBusy || editingId !== null || visibleDevices.length < 2) return;
    const k = e.key;
    if (keyDrag && keyDrag.id !== d.id) {
      // 焦点挪到别的手柄：先放弃未提交的草稿，再让这一下照常开始新的拿起。
      cancelReorder();
    }
    if (k === ' ' || k === 'Enter') {
      e.preventDefault();
      e.stopPropagation();
      if (keyDrag?.id === d.id) {
        const ids = keyDrag.ids;
        const id = keyDrag.id;
        const start = keyDrag.startIds;
        keyDrag = null;
        if (!sameOrder(ids, start)) void commitOrder(ids, id, true);
        else announce($t('device.sort.cancelled'));
        return;
      }
      if (drag) return;
      keyDrag = newDraft(d.id);
      announce($t('device.sort.grabbed', { alias: d.alias }));
      return;
    }
    if (!keyDrag || keyDrag.id !== d.id) return;
    if (k === 'ArrowUp' || k === 'ArrowDown') {
      e.preventDefault();
      e.stopPropagation();
      const at = keyDrag.ids.indexOf(d.id);
      const to = k === 'ArrowUp' ? at - 1 : at + 1;
      if (to < 0 || to >= keyDrag.ids.length) return;
      const ids = moveToIndex(keyDrag.ids, d.id, to);
      keyDrag = { ...keyDrag, ids };
      announce(positionText(d.id, ids));
      return;
    }
    if (k === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      cancelReorder('device.sort.cancelled');
      return;
    }
    if (k === 'Tab') {
      // 不拦 Tab：先取消草稿，再让焦点正常移动。
      cancelReorder();
    }
  }

  function onHandleBlur(): void {
    // 焦点离开正在键盘调整的手柄 → 取消（确认走空格 / Enter）。
    if (keyDrag) cancelReorder();
  }

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
  // 命令预览（另一个组件）也变了，可本页的『已启用设备参数』标识不跟着动」。
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

  function onCardKeydown(e: KeyboardEvent, id: string): void {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      selectDevice(id);
    }
  }

  /** 最后连接时间按相对时间显示：刚刚 / N 分钟前 / N 小时前 / N 天前 / N 个月前。 */
  function relativeTimeText(at: number, current: number, tr: Translate): string {
    const minutes = Math.floor((current - at) / 60_000);
    if (minutes < 1) return tr('device.relative.justNow');
    if (minutes < 60) {
      return tr(minutes === 1 ? 'device.relative.minute' : 'device.relative.minutes', {
        n: minutes,
      });
    }
    const hours = Math.floor(minutes / 60);
    if (hours < 24) {
      return tr(hours === 1 ? 'device.relative.hour' : 'device.relative.hours', { n: hours });
    }
    const days = Math.floor(hours / 24);
    if (days < 30) {
      return tr(days === 1 ? 'device.relative.day' : 'device.relative.days', { n: days });
    }
    const months = Math.floor(days / 30);
    return tr(months === 1 ? 'device.relative.month' : 'device.relative.months', { n: months });
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
    rowActionErr = { ...rowActionErr, [d.id]: null };
    busy = { ...busy, [d.id]: true };
    sessionNotice.set(null);
    try {
      await connectDevice(d.id);
    } catch (e) {
      rowActionErr = { ...rowActionErr, [d.id]: { kind: 'connect', error: e } };
    } finally {
      busy = { ...busy, [d.id]: false };
    }
  }

  /** 断开单台（tasks 9.1）：会话结束、槽位释放，状态回到在线（未连接）。 */
  async function onDisconnect(d: Device): Promise<void> {
    rowActionErr = { ...rowActionErr, [d.id]: null };
    busy = { ...busy, [d.id]: true };
    try {
      await disconnectDevice(d.id);
    } catch (e) {
      rowActionErr = { ...rowActionErr, [d.id]: { kind: 'disconnect', error: e } };
    } finally {
      busy = { ...busy, [d.id]: false };
    }
  }

  /** 全部连接（tasks 9.2）：只提交在线且未连接的设备，离线设备不会被尝试。 */
  async function onConnectAll(): Promise<void> {
    toolBusy = true;
    rowErr = {};
    rowActionErr = {};
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
    rowActionErr = {};
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
    // 编辑期间冻结顺序：状态/别名推送触发的自动重排不得把输入框挪走。
    frozenIds = visibleIds.slice();
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
    try {
      if (cancelled || value === d.alias) return;
      if (!value) {
        rowErr = { ...rowErr, [d.id]: { code: 'device.aliasEmpty' } };
        return;
      }
      // 改名立即落盘（tasks 6.2）：后端返回落盘后的整份配置，前端整体替换，重启后仍是新别名。
      await renameDevice(d.id, value);
    } catch (e) {
      rowErr = { ...rowErr, [d.id]: e };
    } finally {
      // 提交或取消后恢复派生排序。
      frozenIds = null;
    }
  }

  // ── 手动修改无线端口 ───────────────────────────────────────

  function startPortEdit(d: Device): void {
    rowErr = { ...rowErr, [d.id]: null };
    portEditId = d.id;
    portValue = d.port;
    portCancelled = false;
  }

  function onPortKeydown(e: KeyboardEvent): void {
    if (e.key === 'Enter') {
      (e.target as HTMLInputElement).blur();
    } else if (e.key === 'Escape') {
      // 与别名一致：先立取消标记再失焦，避免 blur 把半截输入提交上去。
      portCancelled = true;
      (e.target as HTMLInputElement).blur();
    }
  }

  async function commitPortEdit(d: Device): Promise<void> {
    const value = Number(portValue);
    const cancelled = portCancelled;
    portEditId = null;
    portCancelled = false;
    if (cancelled || value === d.port) return;
    if (!Number.isInteger(value) || value < 1 || value > 65535) {
      rowErr = { ...rowErr, [d.id]: { code: 'wireless.editPort.invalid' } };
      return;
    }
    try {
      // 只写回记录，不自动建链；成功后用返回的配置刷新列表（连接标识随之更新）。
      await setWifiPort(d.id, value);
    } catch (e) {
      rowErr = { ...rowErr, [d.id]: e };
    }
  }

  // ── 删除 ───────────────────────────────────────────────────

  function askDelete(d: Device): void {
    pendingDelete = d;
  }

  function cancelDelete(): void {
    if (delBusy) return;
    pendingDelete = null;
  }

  async function confirmDelete(): Promise<void> {
    const d = pendingDelete;
    if (!d) return;
    delBusy = true;
    rowActionErr = { ...rowActionErr, [d.id]: null };
    try {
      // 别名映射、参数覆盖与覆盖开关由后端一并清除（tasks 6.5）。
      await deleteDevice(d.id);
      pendingDelete = null;
    } catch (e) {
      rowActionErr = { ...rowActionErr, [d.id]: { kind: 'delete', error: e } };
      pendingDelete = null;
    } finally {
      delBusy = false;
    }
  }

  // ── 无线连接（spec `device-management`：配对 / 局域网发现 / 端口解析）──
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
  <DeviceSortSelect
    value={$sortMode}
    options={sortOptions}
    label={$t('device.sort.aria')}
    disabled={sortLocked || editingId !== null}
    on:change={onSortModeChange}
    on:toggle={onSortDirectionToggle}
  />
  <div class="seg dev-density" role="group" aria-label={$t('device.density.label')} use:segSlide>
    <button
      type="button"
      class:active={density === 'detailed'}
      aria-pressed={density === 'detailed'}
      aria-label={$t('device.density.detailed')}
      disabled={densityLocked}
      use:tip={$t('device.density.detailedTip')}
      on:click={() => setDensity('detailed')}
    >
      <LayoutList />
    </button>
    <button
      type="button"
      class:active={density === 'compact'}
      aria-pressed={density === 'compact'}
      aria-label={$t('device.density.compact')}
      disabled={densityLocked}
      use:tip={$t('device.density.compactTip')}
      on:click={() => setDensity('compact')}
    >
      <StretchHorizontal />
    </button>
  </div>
  <div class="spacer"></div>
  {#if summary}
    <span class="status-text" use:tip={statusError || null}>{summary}</span>
  {/if}
  <!-- 批量操作的结果**就地**呈现在本行内（不另起一行）：与设备概况并排，概况在前、结果在后。
       两者语义不同（前者是探测异常的长驻提醒、后者是动作回执），故并排而不互相覆盖。 -->
  {#if $sessionNotice}
    <span class="status-text">{noticeText}</span>
  {/if}
  {#if sortError}
    <span class="status-text offline">{sortError}</span>
  {/if}
  <span class="sr-only" aria-live="polite">{sortAnnouncement}</span>
</div>

{#if $devices.length === 0}
  <div class="card col2" style="text-align:center;color:var(--text-faint);padding:40px 20px">
    {$t('device.empty')}
  </div>
{:else}
  <div
    class="dev-list"
    bind:this={listEl}
    class:drop-end={drag?.active && drag.dropBefore === null}
  >
    {#each visibleDevices as d (d.id)}
      {@const s = $deviceStatus[d.id] ?? 'unknown'}
      {@const sess = sessionByDevice[d.id]}
      <div
        class="dev-card"
        class:compact={compactView}
        class:sel={$selectedDeviceId === d.id}
        class:connected={s === 'connected'}
        class:offline={s === 'offline'}
        class:dragging={drag?.active && drag.id === d.id}
        class:drop-before={drag?.active && drag.dropBefore === d.id}
        data-device-id={d.id}
      >
        <div
          class="dev-main"
          role="button"
          tabindex="0"
          on:click={() => selectDevice(d.id)}
          on:keydown={(e) => onCardKeydown(e, d.id)}
        >
          <button
            type="button"
            bind:this={handleEls[d.id]}
            class="dev-handle"
            class:active={drag?.id === d.id || keyDrag?.id === d.id}
            class:busy={sortBusy}
            disabled={visibleDevices.length < 2 || editingId !== null}
            aria-disabled={sortBusy || undefined}
            aria-label={$t('device.sort.handle', { alias: d.alias })}
            on:pointerdown={(e) => onHandlePointerDown(e, d)}
            on:keydown={(e) => onHandleKeydown(e, d)}
            on:blur={onHandleBlur}
            on:click|stopPropagation
            on:dblclick|stopPropagation
          >
            <GripVertical />
          </button>
          <span class="status-dot {s}"></span>
          <div class="dev-id">
            <div class="dev-alias">
              {#if editingId === d.id}
                <input
                  type="text"
                  bind:value={editValue}
                  use:focusAndSelect
                  on:blur={() => commitEdit(d)}
                  on:keydown={onEditKeydown}
                  aria-label={$t('device.aliasAria')}
                />
              {:else}
                <span class="a-text">{d.alias}</span>
                {#if !compactView}
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
              {/if}
              {#if compactView}
                <!-- 简洁模式把连接方式标识与问题图标留在名称行：详情行整行不渲染。 -->
                <span class="tag {d.kind}">
                  {#if d.kind === 'usb'}<Usb />{:else}<Wifi />{/if}
                  {d.kind === 'usb' ? 'USB' : 'WiFi'}
                </span>
                {#each issuesByDevice[d.id] ?? [] as issue (issue.label)}
                  <DeviceIssueBadge
                    label={issue.label}
                    detail={issue.detail}
                    tone={issue.tone}
                    iconOnly
                  />
                {/each}
              {:else if isOverrideActive(ovById[d.id])}
                <!-- 两态标识：只在"有覆盖取值且开关启用"时出现，MUST NOT 带项数
                     （判据在 store 的 `isOverrideActive`）。禁用态与"一项取值都没填"
                     都不在此表达 —— 前者只在展开面板的标题里说。 -->
                <span class="tag soft">
                  {$t('device.overrideActive')}
                </span>
              {/if}
            </div>
            {#if !compactView}
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
              {#if portEditId === d.id}
                <span class="dev-cid">{d.ip}:</span>
                <input
                  class="dev-cid-input"
                  type="number"
                  min="1"
                  max="65535"
                  step="1"
                  bind:value={portValue}
                  use:focusAndSelect
                  on:blur={() => commitPortEdit(d)}
                  on:keydown={onPortKeydown}
                  on:click|stopPropagation
                  aria-label={$t('wireless.editPort')}
                />
              {:else}
                <span class="dev-cid">
                  {d.connectionId || $t('device.connectionPending')}
                </span>
                {#if s === 'offline' && d.kind === 'wifi'}
                  <span class="tip-host" use:tip={$t('wireless.editPortTip')}>
                    <button
                      type="button"
                      class="cid-edit"
                      aria-label={$t('wireless.editPort')}
                      on:click|stopPropagation={() => startPortEdit(d)}
                    >
                      <Pencil />
                    </button>
                  </span>
                {/if}
              {/if}
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
              {#if s !== 'connected' && d.lastSessionStartedAt}
                <!-- 已连接时不在卡上重复会话现场：PID 与窗口坐标在会话启动日志（含实际下发的
                     --window-x / --window-y）与「日志」页的运行会话里可查。 -->
                <span>·</span>
                <span class="sess-hint">
                  {$t('device.lastConnectedAt', {
                    time: relativeTimeText(d.lastSessionStartedAt, now, $t),
                  })}
                </span>
              {/if}
              {#if rowErr[d.id]}
                <span class="status-text offline">{errorTextOf(rowErr[d.id], $t)}</span>
              {/if}
              <!-- 动作失败与连接警告只留短标签；完整原因进提示层，不参与行布局。 -->
              {#each issuesByDevice[d.id] ?? [] as issue (issue.label)}
                <DeviceIssueBadge label={issue.label} detail={issue.detail} tone={issue.tone} />
              {/each}
            </div>
          {/if}
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
            {#if !compactView && sess}
              <button
                class="btn sm ghost"
                on:click|stopPropagation={() => openLog(d.id)}
                use:tip={$t('device.logTip')}
              >
                <ScrollText /> {$t('device.log')}
              </button>
            {/if}
            <button
              class="btn sm ghost"
              on:click|stopPropagation={() => openParams(d.id)}
              use:tip={$t('device.paramsTip')}
            >
              <SlidersHorizontal /> {$t('device.params')}
            </button>
            {#if !compactView}
              <button
                class="btn sm danger"
                on:click|stopPropagation={() => askDelete(d)}
                use:tip={$t('device.deleteTip')}
              >
                <Trash2 /> {$t('device.delete')}
              </button>
            {/if}
          </div>
        </div>
      </div>
    {/each}
  </div>
{/if}

<!-- 删除确认（spec「设备记录的增删」：删除前给出确认，避免误删） -->
{#if pendingDelete}
  <div class="overlay show" transition:fade={DIALOG_FADE}>
    <div class="overlay-backdrop" role="presentation" on:click={cancelDelete}></div>
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
        <button class="btn ghost" on:click={cancelDelete} disabled={delBusy}>
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
