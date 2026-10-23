<script lang="ts">
  // 添加设备：四种添加方式共用一个弹窗；USB / 手动地址写入设备，扫描 / 配对沿用无线命令。
  import { createEventDispatcher } from 'svelte';
  import { fade, scale } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { t, translateNow } from '../lib/i18n';
  import { errorTextOf } from '../lib/message';
  import { addDevice, devices, refreshDeviceStatus } from '../lib/store';
  import {
    cancelWirelessScan,
    joinWireless,
    pairWireless,
    scanPairingTargets,
    scanWireless,
    scanWirelessPorts,
  } from '../lib/api';
  import type { Device, DiscoveredDevice, PairingTarget } from '../lib/types';
  import HelpTip from './HelpTip.svelte';
  import { KeyRound, RefreshCw, Search, TabletSmartphone, X } from 'lucide-svelte';

  export let open = false;

  const dispatch = createEventDispatcher<{ close: void }>();
  const reduceMotion =
    typeof window !== 'undefined' &&
    !!window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
  const fadeParams = { duration: reduceMotion ? 1 : 180 };
  const scaleParams = { duration: reduceMotion ? 1 : 200, start: 0.94, easing: cubicOut };

  type Method = 'usb' | 'address' | 'discover' | 'pair';
  let method: Method = 'usb';
  let alias = '';
  let serial = '';
  let ip = '';
  let port = 5555;
  let formErr: unknown = null;
  let saving = false;

  type ScanPhase = 'idle' | 'scanning' | 'done' | 'error';
  let phase: ScanPhase = 'idle';
  let found: DiscoveredDevice[] = [];
  let scanErr: unknown = null;
  let mdnsErr: unknown = null;
  let sweepPhase: 'idle' | 'running' | 'done' = 'idle';
  let sweepErr: unknown = null;
  let busyId: string | null = null;
  let joinErrId: string | null = null;
  let joinErr: unknown = null;
  let joinedText = '';
  let scanAliases: Record<string, string> = {};

  type PairPhase = 'idle' | 'detecting' | 'ready' | 'none' | 'error';
  let pairPhase: PairPhase = 'idle';
  let pairTargets: PairingTarget[] = [];
  let pairDetectErr: unknown = null;
  let pairAddress = '';
  let pairCode = '';
  let pairAddressAuto = false;
  let pairAddressRevision = 0;
  let pairing = false;
  let pairFormErr: unknown = null;
  let closing = false;

  type ProbeKind = 'scan' | 'sweep' | 'detect';
  interface ProbeToken {
    id: number;
    kind: ProbeKind;
    epoch: number;
    view: number;
    cancelled: boolean;
  }
  let probe: ProbeToken | null = null;
  let pendingDetect = false;
  let cancelRequested = false;
  let epoch = 0;
  let view = 0;
  let sequence = 0;

  $: writeLocked = saving || pairing || busyId !== null;
  $: actionLocked = writeLocked || !!probe || pendingDetect;
  $: scanCancelable = !!probe && probe.kind === 'scan' && !cancelRequested;
  $: scanWaiting = !!probe && (cancelRequested || probe.kind === 'detect');
  $: pairWaiting = pendingDetect || (!!probe && probe.kind !== 'detect');
  $: addBlocked = method === 'address' && ip.trim() === '';
  $: pairCodeReady = /^\d{6}$/.test(pairCode.trim());
  $: pairListVisible =
    pairTargets.length > 1 ||
    (pairTargets.length === 1 && pairAddress.trim() !== pairTargets[0]?.connectionId);

  $: if (open) enterDialog();

  function enterDialog(): void {
    closing = false;
    epoch++;
    view++;
    pendingDetect = false;
    if (probe) cancelCurrentProbe();
    resetDialog();
  }

  function resetDialog(): void {
    method = 'usb';
    alias = '';
    serial = '';
    ip = '';
    port = 5555;
    formErr = null;
    saving = false;
    resetScan();
    resetPair();
  }

  function resetScan(): void {
    phase = 'idle';
    found = [];
    scanErr = null;
    mdnsErr = null;
    sweepPhase = 'idle';
    sweepErr = null;
    busyId = null;
    joinErrId = null;
    joinErr = null;
    joinedText = '';
    scanAliases = {};
  }

  function keepScanAliases(rows: DiscoveredDevice[]): void {
    const ids = new Set(rows.map((d) => d.connectionId));
    scanAliases = Object.fromEntries(
      Object.entries(scanAliases).filter(([id]) => ids.has(id)),
    );
  }

  function onScanAliasInput(id: string, event: Event): void {
    scanAliases = {
      ...scanAliases,
      [id]: (event.currentTarget as HTMLInputElement).value,
    };
  }

  function resetPair(): void {
    pairPhase = 'idle';
    pairTargets = [];
    pairDetectErr = null;
    pairAddress = '';
    pairCode = '';
    pairAddressAuto = false;
    pairAddressRevision++;
    pairing = false;
    pairFormErr = null;
  }

  function beginProbe(kind: ProbeKind): ProbeToken | null {
    if (!open || probe || writeLocked) return null;
    const token: ProbeToken = { id: ++sequence, kind, epoch, view, cancelled: false };
    probe = token;
    cancelRequested = false;
    return token;
  }

  function isCurrent(token: ProbeToken): boolean {
    return probe === token && token.epoch === epoch && token.view === view;
  }

  function finishProbe(token: ProbeToken): void {
    if (probe !== token) return;
    const stale = token.epoch !== epoch || token.view !== view;
    probe = null;
    cancelRequested = false;
    if (open && stale) {
      if ((token.kind === 'scan' || token.kind === 'sweep') && phase === 'scanning') {
        phase = found.length ? 'done' : 'idle';
      }
      if (token.kind === 'sweep' && sweepPhase === 'running') {
        sweepPhase = found.length ? 'done' : 'idle';
      }
      if (token.kind === 'detect' && pairPhase === 'detecting') {
        pairPhase = 'idle';
      }
    }
    pumpPending();
  }

  function pumpPending(): void {
    if (!open || !pendingDetect) return;
    if (method !== 'pair') {
      pendingDetect = false;
      return;
    }
    if (writeLocked || probe) return;
    pendingDetect = false;
    void startDetect();
  }

  function cancelCurrentProbe(): void {
    if (!probe || probe.cancelled) return;
    probe.cancelled = true;
    cancelRequested = true;
    void cancelWirelessScan().catch(() => {
      /* 取消失败也要等原请求收尾 */
    });
  }

  function close(): void {
    if (writeLocked || closing) return;
    closing = true;
    epoch++;
    view++;
    pendingDetect = false;
    if (probe) cancelCurrentProbe();
    dispatch('close');
  }

  function onWindowKeydown(event: KeyboardEvent): void {
    if (!open || event.key !== 'Escape') return;
    event.preventDefault();
    close();
  }

  function selectMethod(next: Method): void {
    if (writeLocked || method === next) return;
    method = next;
    view++;
    pendingDetect = false;
    if (probe) cancelCurrentProbe();
    if (next === 'pair') {
      if (probe) pendingDetect = true;
      else void startDetect();
    }
  }

  function onPairAddressInput(): void {
    pairAddressAuto = false;
    pairAddressRevision++;
  }

  function onPairTargetChange(): void {
    pairAddressAuto = false;
    pairAddressRevision++;
  }

  async function submitAdd(): Promise<void> {
    if (writeLocked || (method !== 'usb' && method !== 'address')) return;
    formErr = null;
    saving = true;
    try {
      const device: Device = {
        id: '',
        alias:
          alias.trim() ||
          translateNow('device.defaultAlias', { count: $devices.length + 1 }),
        kind: method === 'usb' ? 'usb' : 'wifi',
        serial: method === 'usb' ? serial.trim() : '',
        ip: method === 'address' ? ip.trim() : '',
        port: method === 'address' ? Number(port) || 5555 : 0,
      };
      await addDevice(device);
      // 新设备的 id 此前不在状态表里；主动取一次，别等只随 adb 变化推送的事件。
      await refreshDeviceStatus().catch(() => {
        /* 添加已成功；状态会由后续推送补上 */
      });
      closing = true;
      dispatch('close');
    } catch (e) {
      formErr = e;
    } finally {
      saving = false;
    }
  }

  async function runScan(): Promise<void> {
    if (method !== 'discover') return;
    const token = beginProbe('scan');
    if (!token) return;
    phase = 'scanning';
    scanErr = null;
    mdnsErr = null;
    found = [];
    joinedText = '';
    try {
      const result = await scanWireless();
      if (!isCurrent(token)) return;
      found = result.devices;
      keepScanAliases(found);
      mdnsErr = result.mdnsError;
      phase = 'done';
    } catch (e) {
      if (isCurrent(token)) {
        phase = 'error';
        scanErr = e;
      }
    } finally {
      finishProbe(token);
    }
  }

  async function runSweep(): Promise<void> {
    if (method !== 'discover') return;
    const token = beginProbe('sweep');
    if (!token) return;
    sweepPhase = 'running';
    sweepErr = null;
    try {
      const list = await scanWirelessPorts();
      if (!isCurrent(token)) return;
      const seen = new Set(found.map((d) => d.connectionId));
      found = [...found, ...list.filter((d) => !seen.has(d.connectionId))];
      keepScanAliases(found);
      sweepPhase = 'done';
    } catch (e) {
      if (isCurrent(token)) {
        sweepErr = e;
        sweepPhase = 'done';
      }
    } finally {
      finishProbe(token);
    }
  }

  async function onJoin(device: DiscoveredDevice): Promise<void> {
    if (writeLocked || probe || pendingDetect) return;
    busyId = device.connectionId;
    joinErrId = null;
    joinErr = null;
    joinedText = '';
    const rowAlias = scanAliases[device.connectionId]?.trim();
    try {
      const joined = await joinWireless({
        ip: device.ip,
        port: device.port,
        ...(rowAlias ? { alias: rowAlias } : {}),
      });
      await refreshDeviceStatus();
      found = found.map((d) =>
        d.ip === device.ip ? { ...d, registered: true } : d,
      );
      joinedText = $t('wireless.discover.joined', { alias: joined.alias });
    } catch (e) {
      joinErrId = device.connectionId;
      joinErr = e;
    } finally {
      busyId = null;
    }
  }

  async function startDetect(): Promise<void> {
    if (!open || method !== 'pair' || writeLocked || probe) return;
    pendingDetect = false;
    const token = beginProbe('detect');
    if (!token) return;
    pairPhase = 'detecting';
    pairTargets = [];
    pairDetectErr = null;
    const revision = pairAddressRevision;
    try {
      const list = await scanPairingTargets();
      if (!isCurrent(token)) return;
      pairTargets = list;
      if (list.length === 0) {
        if (pairAddressAuto && revision === pairAddressRevision) {
          pairAddress = '';
          pairAddressAuto = false;
        }
        pairPhase = 'none';
        return;
      }
      pairPhase = 'ready';
      if (list.length === 1) {
        if (revision === pairAddressRevision && (pairAddress.trim() === '' || pairAddressAuto)) {
          pairAddress = list[0].connectionId;
          pairAddressAuto = true;
        }
      } else if (pairAddressAuto && revision === pairAddressRevision) {
        pairAddress = '';
        pairAddressAuto = false;
      }
    } catch (e) {
      if (isCurrent(token)) {
        pairPhase = 'error';
        pairDetectErr = e;
      }
    } finally {
      finishProbe(token);
    }
  }

  async function onPair(): Promise<void> {
    if (writeLocked || probe || pendingDetect) return;
    pairFormErr = null;
    pairing = true;
    const trimmedAlias = alias.trim();
    try {
      await pairWireless({
        address: pairAddress.trim(),
        code: pairCode.trim(),
        ...(trimmedAlias ? { alias: trimmedAlias } : {}),
      });
      await refreshDeviceStatus();
      closing = true;
      dispatch('close');
    } catch (e) {
      pairFormErr = e;
    } finally {
      pairing = false;
    }
  }

  function measureSize(node: HTMLElement): { destroy(): void } {
    const shell = node.closest('.add-device-modal') as HTMLElement | null;
    if (!shell) return { destroy() {} };
    const measure = (first = false): void => {
      const css = getComputedStyle(shell);
      shell.style.overflowY = 'hidden';
      const extra =
        (parseFloat(css.paddingTop) || 0) +
        (parseFloat(css.paddingBottom) || 0) +
        (parseFloat(css.borderTopWidth) || 0) +
        (parseFloat(css.borderBottomWidth) || 0);
      const natural = Math.ceil(node.offsetHeight + extra);
      const limit = parseFloat(css.maxHeight);
      if (Number.isFinite(limit) && natural > limit) shell.style.overflowY = 'auto';
      else shell.scrollTop = 0;
      const height = `${natural}px`;
      if (first) shell.style.transition = 'none';
      if (shell.style.height !== height) shell.style.height = height;
      if (first) {
        void shell.offsetHeight;
        shell.style.removeProperty('transition');
      }
    };
    const observer = new ResizeObserver(() => measure());
    observer.observe(node);
    const onResize = (): void => measure();
    window.addEventListener('resize', onResize);
    measure(true);
    return {
      destroy(): void {
        observer.disconnect();
        window.removeEventListener('resize', onResize);
      },
    };
  }
</script>

<svelte:window on:keydown={onWindowKeydown} />

{#if open}
  <div
    class="overlay show add-overlay"
    class:closing
    transition:fade={fadeParams}
  >
    <div
      class="modal add-device-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="add-device-title"
      transition:scale={scaleParams}
    >
      <div class="add-device-inner" use:measureSize>
      <button
        class="add-close"
        type="button"
        aria-label={$t('wireless.close')}
        disabled={writeLocked}
        on:click={close}
      >
        <X />
      </button>
      <div class="ico"><TabletSmartphone /></div>
      <h2 id="add-device-title">{$t('device.add.title')}</h2>

      <div class="field">
        <span class="flabel" id="add-method-label">{$t('device.add.method')}</span>
        <div class="seg add-methods" role="group" aria-labelledby="add-method-label">
          <button
            type="button"
            class:active={method === 'usb'}
            aria-pressed={method === 'usb'}
            disabled={writeLocked}
            on:click={() => selectMethod('usb')}
          >
            <span>{$t('device.add.method.usb')}</span><span class="method-detail">(serial)</span>
          </button>
          <button
            type="button"
            class:active={method === 'address'}
            aria-pressed={method === 'address'}
            disabled={writeLocked}
            on:click={() => selectMethod('address')}
          >
            <span>{$t('device.add.method.manual')}</span><span class="method-detail">(IP:Port)</span>
          </button>
          <button
            type="button"
            class:active={method === 'discover'}
            aria-pressed={method === 'discover'}
            disabled={writeLocked}
            on:click={() => selectMethod('discover')}
          >
            <span>{$t('device.add.method.discover')}</span>
          </button>
          <button
            type="button"
            class:active={method === 'pair'}
            aria-pressed={method === 'pair'}
            disabled={writeLocked}
            on:click={() => selectMethod('pair')}
          >
            <span>{$t('wireless.pair.title')}</span>
          </button>
        </div>
      </div>

      {#if method !== 'discover'}
        <div class="field">
          <label for="add-alias">{$t('device.add.alias')}</label>
          <input
            id="add-alias"
            type="text"
            bind:value={alias}
            disabled={writeLocked}
            placeholder={$t('device.add.aliasPlaceholder')}
          />
        </div>
      {/if}

      {#if method === 'usb'}
        <div class="field">
          <span class="flabel" id="add-serial-label">
            {$t('device.add.serial')}
            <HelpTip
              title={$t('device.add.serial')}
              label={$t('common.fieldHelp', { field: $t('device.add.serial') })}
            >
              <span class="fhelp-p">{$t('device.add.serialHint')}</span>
            </HelpTip>
          </span>
          <input
            id="add-serial"
            type="text"
            bind:value={serial}
            disabled={writeLocked}
            aria-labelledby="add-serial-label"
            placeholder={$t('device.add.serialPlaceholder')}
          />
        </div>
        {#if formErr}
          <div class="hint dl-error">{errorTextOf(formErr, $t)}</div>
        {/if}
      {:else if method === 'address'}
        <div class="field">
          <span class="flabel" id="add-ip-label">
            {$t('device.add.ip')}
            <HelpTip
              title={$t('device.add.ip')}
              label={$t('common.fieldHelp', { field: $t('device.add.ip') })}
            >
              <span class="fhelp-p">{$t('device.add.wifiHint')}</span>
            </HelpTip>
          </span>
          <div class="row">
            <input
              id="add-ip"
              type="text"
              bind:value={ip}
              disabled={writeLocked}
              aria-labelledby="add-ip-label"
              placeholder={$t('device.add.ipPlaceholder')}
            />
            <input
              class="add-port"
              type="number"
              bind:value={port}
              min="1"
              max="65535"
              disabled={writeLocked}
              aria-label={$t('device.add.portAria')}
            />
          </div>
        </div>
        {#if formErr}
          <div class="hint dl-error">{errorTextOf(formErr, $t)}</div>
        {/if}
      {:else if method === 'discover'}
        <div class="wl-head">
          <span class="flabel">
            {$t('wireless.discover.title')}
            <HelpTip
              title={$t('wireless.discover.title')}
              label={$t('common.fieldHelp', { field: $t('wireless.discover.title') })}
            >
              <span class="fhelp-p">{$t('wireless.discover.authHint')}</span>
              <span class="fhelp-p">{$t('wireless.discover.sweepHint')}</span>
            </HelpTip>
          </span>
          <div class="wl-head-actions">
            {#if scanCancelable}
              <button
                class="btn sm ghost"
                type="button"
                disabled={writeLocked}
                on:click={cancelCurrentProbe}
              >
                {$t('wireless.discover.cancel')}
              </button>
            {:else}
              <button
                class="btn sm"
                type="button"
                on:click={runScan}
                disabled={actionLocked}
              >
                <Search />
                {phase === 'done'
                  ? $t('wireless.discover.rescan')
                  : $t('wireless.discover.scan')}
              </button>
            {/if}
          </div>
        </div>
        <div class="wl-status" role="status" aria-live="polite">
          {#if scanWaiting}
            {$t('wireless.probeWaiting')}
          {:else if phase === 'idle'}
            {$t('wireless.discover.idle')}
          {:else if phase === 'scanning'}
            <span class="status-spinner" aria-hidden="true"></span>{$t('wireless.discover.scanning')}
          {:else if phase === 'error'}
            <span class="wl-err">{errorTextOf(scanErr, $t)}</span>
          {:else if found.length > 0}
            {$t('wireless.discover.count', { count: found.length })}
          {:else if !mdnsErr}
            {$t('wireless.discover.empty')}
          {/if}
        </div>
        {#if mdnsErr}
          <div class="hint wl-err">{errorTextOf(mdnsErr, $t)}</div>
        {/if}
        {#if found.length > 0}
          <ul class="wl-list">
            {#each found as d (d.connectionId)}
              <li class="wl-item wl-result" data-connection-id={d.connectionId}>
                <div class="wl-result-head">
                  <span class="wl-cid">{d.connectionId}</span>
                  {#if d.registered}
                    <span class="tag over">{$t('wireless.discover.registered')}</span>
                  {/if}
                </div>
                {#if !d.registered}
                  <div class="wl-alias-row">
                    <input
                      type="text"
                      value={scanAliases[d.connectionId] ?? ''}
                      data-scan-alias={d.connectionId}
                      disabled={writeLocked}
                      on:input={(e) => onScanAliasInput(d.connectionId, e)}
                      placeholder={$t('device.add.alias')}
                      aria-label={$t('device.add.alias')}
                    />
                    <button
                      class="btn sm primary"
                      type="button"
                      data-join={d.connectionId}
                      on:click={() => onJoin(d)}
                      disabled={actionLocked}
                    >
                      {busyId === d.connectionId
                        ? $t('wireless.discover.joining')
                        : $t('wireless.discover.join')}
                    </button>
                  </div>
                {/if}
              </li>
              {#if joinErrId === d.connectionId && joinErr}
                <li class="wl-row-err">{errorTextOf(joinErr, $t)}</li>
              {/if}
            {/each}
          </ul>
        {/if}
        {#if joinedText}
          <div class="hint wl-ok">{joinedText}</div>
        {/if}
        {#if (phase === 'done' && found.length === 0) || mdnsErr || sweepPhase !== 'idle'}
          <div class="wl-sweep">
            <button
              class="btn sm ghost"
              type="button"
              on:click={runSweep}
              disabled={actionLocked}
            >
              <RefreshCw />
              {sweepPhase === 'running'
                ? $t('wireless.discover.sweeping')
                : $t('wireless.discover.sweep')}
            </button>
            {#if sweepErr}
              <div class="hint wl-err">{errorTextOf(sweepErr, $t)}</div>
            {/if}
          </div>
        {/if}
      {:else}
        <div class="field">
          <div class="wl-head">
            <label class="flabel pair-address-label" for="add-pair-address">{$t('wireless.pair.address')}</label>
            <div class="wl-head-actions">
              <button
                class="btn sm ghost"
                type="button"
                on:click={startDetect}
                disabled={actionLocked}
              >
                <RefreshCw /> {$t('wireless.pair.redetect')}
              </button>
            </div>
          </div>
          <input
            id="add-pair-address"
            type="text"
            bind:value={pairAddress}
            disabled={writeLocked}
            on:input={onPairAddressInput}
            placeholder={$t('wireless.pair.addressPlaceholder')}
            aria-describedby="add-pair-status"
          />
          <div
            class="wl-status pair-status"
            id="add-pair-status"
            role="status"
            aria-live="polite"
          >
            {#if pairWaiting}
              {$t('wireless.probeWaiting')}
            {:else if pairPhase === 'detecting'}
              <span class="status-spinner" aria-hidden="true"></span>{$t('wireless.pair.detecting')}
            {:else if pairPhase === 'error'}
              <span class="wl-err">{errorTextOf(pairDetectErr, $t)}</span>
            {:else if pairPhase === 'none'}
              {$t('wireless.pair.none')}
            {:else if pairTargets.length === 1}
              {$t('wireless.pair.foundReady')}
            {:else if pairTargets.length > 1}
              {$t('wireless.pair.multiple', { count: pairTargets.length })}
            {/if}
          </div>
          {#if pairListVisible}
            <ul class="wl-list">
              {#each pairTargets as target (target.connectionId)}
                <li class="wl-item">
                  <label class="wl-radio">
                    <input
                      type="radio"
                      name="add-pair-target"
                      value={target.connectionId}
                      bind:group={pairAddress}
                      disabled={writeLocked}
                      on:change={onPairTargetChange}
                    />
                    <span class="wl-cid">{target.connectionId}</span>
                  </label>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
        <div class="field">
          <span class="flabel" id="add-pair-code-label">
            {$t('wireless.pair.code')}
            <HelpTip
              up
              title={$t('wireless.pair.code')}
              label={$t('common.fieldHelp', { field: $t('wireless.pair.code') })}
            >
              <span class="fhelp-p">{$t('wireless.pair.hint')}</span>
            </HelpTip>
          </span>
          <input
            id="add-pair-code"
            type="text"
            inputmode="numeric"
            maxlength="6"
            bind:value={pairCode}
            disabled={writeLocked}
            aria-labelledby="add-pair-code-label"
            placeholder={$t('wireless.pair.codePlaceholder')}
          />
        </div>
        {#if pairFormErr}
          <div class="wl-status wl-err" role="alert">{errorTextOf(pairFormErr, $t)}</div>
        {/if}
      {/if}

      {#if method !== 'discover'}
        <div class="actions">
          {#if method === 'pair'}
            <button
              class="btn primary"
              type="button"
              on:click={onPair}
              disabled={actionLocked || pairAddress.trim() === '' || !pairCodeReady}
            >
              <KeyRound />
              {pairing ? $t('wireless.pair.pairing') : $t('wireless.pair.submit')}
            </button>
          {:else}
            <button
              class="btn primary"
              type="button"
              on:click={submitAdd}
              disabled={writeLocked || addBlocked}
            >
              {saving ? $t('device.add.saving') : $t('device.add.save')}
            </button>
          {/if}
        </div>
      {/if}
      </div>
    </div>
  </div>
{/if}
