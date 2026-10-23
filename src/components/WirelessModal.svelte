<script lang="ts">
  // 无线连接弹窗（spec `device-management` 的「无线调试的配对」与「局域网无线设备的发现」）。
  //
  // 两段并列而不是两个 tab：两段都很短，且真实路径常常是「先试发现、不行再配对」，
  // 藏起其中一段只会让用户多点一次。
  //
  // 扫描一律由用户触发：这里没有定时器、没有 onMount 自动扫描 —— 打开弹窗只做一件
  // 网络动作（查找待配对设备，4 秒窗口），因为设备的配对弹窗本身就是限时的，
  // 用户打开这个弹窗通常正是为了用它。发现列表必须点「扫描局域网设备」才动。
  import { createEventDispatcher } from 'svelte';
  import { t } from '../lib/i18n';
  import { errorTextOf } from '../lib/message';
  import { refreshDeviceStatus } from '../lib/store';
  import {
    cancelWirelessScan,
    joinWireless,
    pairWireless,
    scanPairingTargets,
    scanWireless,
    scanWirelessPorts,
  } from '../lib/api';
  import type { DiscoveredDevice, PairingTarget } from '../lib/types';
  import { Wifi, Search, KeyRound, RefreshCw } from 'lucide-svelte';

  export let open = false;

  const dispatch = createEventDispatcher<{ close: void }>();

  // ── 发现段 ──
  type ScanPhase = 'idle' | 'scanning' | 'done' | 'error';
  let phase: ScanPhase = 'idle';
  let devices: DiscoveredDevice[] = [];
  let scanErr: unknown = null;
  /** 多播本身不可用的原因（与「扫到了但没设备」是两件事，处置也不同） */
  let mdnsErr: unknown = null;
  let sweepPhase: 'idle' | 'running' | 'done' = 'idle';
  let sweepErr: unknown = null;
  /** 「加入」在途的设备（按连接标识记，避免整列表一起变灰） */
  let busyId: string | null = null;
  let joinErrId: string | null = null;
  let joinErr: unknown = null;
  let joinedText = '';

  // ── 配对段 ──
  type PairPhase = 'detecting' | 'ready' | 'none' | 'error';
  let pairPhase: PairPhase = 'detecting';
  let pairTargets: PairingTarget[] = [];
  let pairDetectErr: unknown = null;
  let pairAddress = '';
  let pairCode = '';
  let pairing = false;
  let pairFormErr: unknown = null;

  /** 打开即重置发现段并查找待配对设备（每次打开都重新查：配对服务只在那几秒里存在）。 */
  $: if (open) {
    resetScan();
    detectTargets();
  }

  function resetScan(): void {
    phase = 'idle';
    devices = [];
    scanErr = null;
    mdnsErr = null;
    sweepPhase = 'idle';
    sweepErr = null;
    busyId = null;
    joinErrId = null;
    joinErr = null;
    joinedText = '';
    pairCode = '';
    pairFormErr = null;
  }

  function close(): void {
    dispatch('close');
  }

  async function detectTargets(): Promise<void> {
    pairPhase = 'detecting';
    pairTargets = [];
    pairDetectErr = null;
    try {
      const list = await scanPairingTargets();
      pairTargets = list;
      if (list.length === 0) {
        pairPhase = 'none';
        return;
      }
      pairPhase = 'ready';
      // 只有一条就直接用；多条交给用户选（猜错会把配对码发给另一台设备）。
      if (!list.some((x) => x.connectionId === pairAddress)) {
        pairAddress = list[0].connectionId;
      }
    } catch (e) {
      pairPhase = 'error';
      pairDetectErr = e;
    }
  }

  async function runScan(): Promise<void> {
    phase = 'scanning';
    scanErr = null;
    mdnsErr = null;
    devices = [];
    joinedText = '';
    try {
      const result = await scanWireless();
      devices = result.devices;
      mdnsErr = result.mdnsError;
      phase = 'done';
    } catch (e) {
      phase = 'error';
      scanErr = e;
    }
  }

  /** 取消：后端在下一拍退出，命令返回后这里自然落到「已完成」。 */
  async function cancelScan(): Promise<void> {
    try {
      await cancelWirelessScan();
    } catch {
      /* 取消失败无需打扰用户：扫描本身也会到点结束 */
    }
  }

  async function runSweep(): Promise<void> {
    sweepPhase = 'running';
    sweepErr = null;
    try {
      const list = await scanWirelessPorts();
      const seen = new Set(devices.map((d) => d.connectionId));
      devices = [...devices, ...list.filter((d) => !seen.has(d.connectionId))];
      sweepPhase = 'done';
    } catch (e) {
      sweepErr = e;
      sweepPhase = 'done';
    }
  }

  async function onJoin(device: DiscoveredDevice): Promise<void> {
    busyId = device.connectionId;
    joinErrId = null;
    joinErr = null;
    joinedText = '';
    try {
      const joined = await joinWireless({ ip: device.ip, port: device.port });
      await refreshDeviceStatus();
      // 同一台设备的其它端口条目一并标为已登记（按 IP 判定，与后端同一口径）。
      devices = devices.map((d) =>
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

  async function onPair(): Promise<void> {
    pairFormErr = null;
    pairing = true;
    try {
      await pairWireless({ address: pairAddress.trim(), code: pairCode.trim() });
      await refreshDeviceStatus();
      // 配对是一次性流程：成功后关闭弹窗，设备已在列表里。
      dispatch('close');
    } catch (e) {
      pairFormErr = e;
    } finally {
      pairing = false;
    }
  }
</script>

{#if open}
  <div class="overlay show">
    <div class="modal wl-modal">
      <div class="ico"><Wifi /></div>
      <h2>{$t('wireless.title')}</h2>
      <p>{$t('wireless.description')}</p>

      <!-- ① 局域网设备 -->
      <section class="wl-sec">
        <div class="wl-head">
          <span class="flabel">{$t('wireless.discover.title')}</span>
          <div class="wl-head-actions">
            {#if phase === 'scanning'}
              <button class="btn sm ghost" on:click={cancelScan}>
                {$t('wireless.discover.cancel')}
              </button>
            {:else}
              <button
                class="btn sm"
                on:click={runScan}
                disabled={sweepPhase === 'running' || pairing}
              >
                <Search />
                {phase === 'done'
                  ? $t('wireless.discover.rescan')
                  : $t('wireless.discover.scan')}
              </button>
            {/if}
          </div>
        </div>

        <div class="wl-status">
          {#if phase === 'idle'}
            {$t('wireless.discover.idle')}
          {:else if phase === 'scanning'}
            {$t('wireless.discover.scanning')}
          {:else if phase === 'error'}
            <span class="wl-err">{errorTextOf(scanErr, $t)}</span>
          {:else if devices.length > 0}
            {$t('wireless.discover.count', { count: devices.length })}
          {:else if !mdnsErr}
            {$t('wireless.discover.empty')}
          {/if}
        </div>

        {#if mdnsErr}
          <div class="hint wl-err">{errorTextOf(mdnsErr, $t)}</div>
        {/if}

        {#if devices.length > 0}
          <ul class="wl-list">
            {#each devices as d (d.connectionId)}
              <li class="wl-item">
                <span class="wl-cid">{d.connectionId}</span>
                {#if d.registered}
                  <span class="tag over">{$t('wireless.discover.registered')}</span>
                {:else}
                  <button
                    class="btn sm primary"
                    on:click={() => onJoin(d)}
                    disabled={busyId !== null || pairing}
                  >
                    {busyId === d.connectionId
                      ? $t('wireless.discover.joining')
                      : $t('wireless.discover.join')}
                  </button>
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

        <!-- 5555 兜底：只在多播不可用、或扫完一台都没有时才露出来 -->
        {#if (phase === 'done' && devices.length === 0) || mdnsErr || sweepPhase !== 'idle'}
          <div class="wl-sweep">
            <div class="hint">{$t('wireless.discover.sweepHint')}</div>
            <button
              class="btn sm ghost"
              on:click={runSweep}
              disabled={sweepPhase === 'running' || busyId !== null}
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
      </section>

      <!-- ② 配对码 -->
      <section class="wl-sec">
        <div class="wl-head">
          <span class="flabel">{$t('wireless.pair.title')}</span>
          <div class="wl-head-actions">
            <button
              class="btn sm ghost"
              on:click={detectTargets}
              disabled={pairPhase === 'detecting' || pairing}
            >
              <RefreshCw /> {$t('wireless.pair.redetect')}
            </button>
          </div>
        </div>
        <div class="hint">{$t('wireless.pair.hint')}</div>

        <div class="wl-status">
          {#if pairPhase === 'detecting'}
            {$t('wireless.pair.detecting')}
          {:else if pairPhase === 'error'}
            <span class="wl-err">{errorTextOf(pairDetectErr, $t)}</span>
          {:else if pairPhase === 'none'}
            {$t('wireless.pair.none')}
          {:else if pairTargets.length === 1}
            {$t('wireless.pair.found', {
              address: pairTargets[0].connectionId,
            })}
          {:else}
            {$t('wireless.pair.multiple', { count: pairTargets.length })}
          {/if}
        </div>

        {#if pairTargets.length > 1}
          <ul class="wl-list">
            {#each pairTargets as target (target.connectionId)}
              <li class="wl-item">
                <label class="wl-radio">
                  <input
                    type="radio"
                    name="wl-pair-target"
                    value={target.connectionId}
                    bind:group={pairAddress}
                  />
                  <span class="wl-cid">{target.connectionId}</span>
                </label>
              </li>
            {/each}
          </ul>
        {/if}

        <div class="field">
          <label for="wl-pair-address">{$t('wireless.pair.address')}</label>
          <input
            id="wl-pair-address"
            type="text"
            bind:value={pairAddress}
            placeholder={$t('wireless.pair.addressPlaceholder')}
          />
          <div class="hint">{$t('wireless.pair.manualHint')}</div>
        </div>
        <div class="field">
          <label for="wl-pair-code">{$t('wireless.pair.code')}</label>
          <div class="row">
            <input
              id="wl-pair-code"
              type="text"
              inputmode="numeric"
              maxlength="10"
              bind:value={pairCode}
              placeholder={$t('wireless.pair.codePlaceholder')}
            />
            <button
              class="btn primary"
              on:click={onPair}
              disabled={pairing || pairCode.trim() === '' || pairAddress.trim() === ''}
            >
              <KeyRound />
              {pairing ? $t('wireless.pair.pairing') : $t('wireless.pair.submit')}
            </button>
          </div>
        </div>

        {#if pairFormErr}
          <div class="hint wl-err">{errorTextOf(pairFormErr, $t)}</div>
        {/if}
      </section>

      <div class="actions">
        <button class="btn ghost" on:click={close}>{$t('wireless.close')}</button>
      </div>
    </div>
  </div>
{/if}

<!-- 样式在 src/app.css 的「无线连接弹窗」一节：与原型同名同值，便于逐条对照。 -->
