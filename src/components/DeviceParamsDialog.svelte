<script lang="ts">
  // 设备参数弹窗（spec `parameter-configuration`「设备参数弹窗」+ `command-preview`
  //「设备参数弹窗内的命令预览」/ design D2–D8）。
  //
  // 三区结构：固定头部（标题 / 启停开关 / 扩大缩小 / 关闭）、可滚动体区、固定底部预览。
  // 只有体区滚动 —— 头部与预览因此永远可见，「改一项就看一眼命令」不必关窗。
  //
  // **预览跟「弹窗所属设备」走，不是当前选中设备**：`effectiveParams` 跟的是
  // `selectedDeviceId`，从列表里给非选中设备打开弹窗时会显示另一台设备的命令，
  // 而且看起来完全正常。故这里按本设备的 id 自己合并（design D3）。
  //
  // 扩大是组件内的状态，随组件销毁复位 —— 父级用 `{#if}` 挂载本组件，因此「每次打开
  // 都回到常态」由挂载/卸载保证，不靠额外的事件去重置（design D4）。
  import { createEventDispatcher } from 'svelte';
  import { fade } from 'svelte/transition';
  import {
    defaults,
    devices,
    overrides,
    overrideErrors,
    setOverrideValue,
    setOverrideEnabled,
    countOverrideKeys,
    isOverrideEnabled,
  } from '../lib/store';
  import { mergeParams, buildCommandParts } from '../lib/command';
  import { validateParams } from '../lib/validate';
  import { OVERRIDE_GROUPS, fieldOf, refKey, type FieldRef, type ParamField as Field } from '../lib/params';
  import type { Defaults, OverrideEntry } from '../lib/types';
  import { t } from '../lib/i18n';
  import { validationText } from '../lib/paramText';
  import { tip } from '../lib/tooltip';
  import ParamField from './ParamField.svelte';
  import { X, Maximize2, Minimize2, SlidersHorizontal, AlertTriangle, Terminal } from 'lucide-svelte';

  export let deviceId: string;

  const dispatch = createEventDispatcher<{ close: void }>();

  /** 扩大态：仅本次打开有效（design D4）。 */
  let expanded = false;
  /** 帮助面板向上展开（design D7）：体区必须滚动，向下展开的面板会被下缘裁掉。 */
  let panelUp = false;
  let previewEl: HTMLElement | null = null;

  /** 覆盖表 / 默认参数 / 覆盖错误：显式取出来给模板直接用（Svelte 只跟踪模板里出现的变量）。 */
  $: ovById = $overrides;
  $: defaultsMap = $defaults as unknown as Record<string, Record<string, unknown>>;
  $: ovErrById = $overrideErrors;

  $: device = $devices.find((d) => d.id === deviceId) ?? null;
  $: entry = ovById[deviceId] as OverrideEntry | undefined;
  $: enabled = isOverrideEnabled(entry);
  $: keptCount = countOverrideKeys(entry?.values as Record<string, unknown>);

  /**
   * 本设备合并后的有效参数：`enabled ? merge(默认, 覆盖) : 默认`。
   *
   * 与 store 的 `mergeForDevice` 同一规则 —— 那边是普通函数、不参与响应式，这里必须在
   * `$:` 里按 `$defaults` / `$overrides` 重算，否则改默认参数后预览与字段显隐不刷新。
   */
  $: effective = mergeParams(
    $defaults as unknown as Record<string, unknown>,
    enabled ? (entry?.values as Record<string, unknown>) : null,
  ) as unknown as Defaults;
  $: effectiveMap = effective as unknown as Record<string, Record<string, unknown>>;

  // 预览的剔除口径与页面底部命令条同源：非法字段不进命令。
  $: previewErrors = validateParams(effectiveMap);
  $: parts = device?.connectionId
    ? buildCommandParts(device.connectionId, effective, previewErrors)
    : null;
  $: errorTexts = Object.values(previewErrors).map((e) => validationText(e, $t));

  /**
   * 弹窗要渲染的字段：分节 + 字段定义 + 是否可见。
   *
   * 先解析成列表再 `{#each}`（与旧面板同一条经验）：在 `{#each}` 里用 `{@const}` 包住
   * 子组件会让属性更新挂在常量 dirty 位上，取值变化时输入框回显不刷新。
   */
  $: sections = OVERRIDE_GROUPS.map((group) => ({
    titleKey: group.titleKey,
    span: !!group.span,
    fields: group.fields
      .map((ref) => ({ ref, field: fieldOf(ref.group, ref.key) as Field | undefined }))
      .filter(
        (x): x is { ref: FieldRef; field: Field } => {
          if (!x.field) return false;
          // 字段显隐读**合并后的有效值**（design D6），不读覆盖值 —— 按覆盖值判定会出现
          //「控件显示着，但命令里根本没有这个 flag」。
          return !x.field.showWhen || x.field.showWhen(effectiveMap[x.ref.group] ?? {});
        },
      ),
  })).filter((s) => s.fields.length > 0);

  /** 字段取值：有覆盖取覆盖值（禁用时也回显），否则回落默认值。 */
  function displayOf(ref: FieldRef): unknown {
    const group = (entry?.values as Record<string, Record<string, unknown>> | undefined)?.[ref.group];
    if (group && ref.key in group) return group[ref.key];
    return defaultsMap[ref.group]?.[ref.key];
  }

  function fieldError(ref: FieldRef) {
    return ovErrById[deviceId]?.[refKey(ref.group, ref.key)] ?? null;
  }

  // ── 预览横向浏览（与命令条同一手法：滚轮转 scrollLeft）──
  function onPreviewWheel(e: WheelEvent): void {
    const el = previewEl;
    if (!el || el.scrollWidth <= el.clientWidth + 1) return;
    const raw = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY;
    const amount = e.deltaMode === 1 ? raw * 16 : e.deltaMode === 2 ? raw * el.clientWidth : raw;
    const max = el.scrollWidth - el.clientWidth;
    el.scrollLeft = Math.max(0, Math.min(max, el.scrollLeft + amount));
    e.preventDefault();
  }

  // ── 帮助面板择向（design D7）──
  //
  // 同一时刻最多一块 `?` 面板展开（由焦点唯一性保证），所以用一个体区级的标记翻转**当前**
  // 那一块即可，不必给每个 ParamField 加属性。阈值取 220px（与 ParamSelect 择向的口径同档）：
  // 低于它时下方放不下一块有意义的说明。
  const PANEL_ROOM = 220;

  function trackPanelSpace(e: Event): void {
    const body = e.currentTarget as HTMLElement;
    const field = (e.target as HTMLElement | null)?.closest?.('.field') as HTMLElement | null;
    if (!field) return;
    panelUp = body.getBoundingClientRect().bottom - field.getBoundingClientRect().bottom < PANEL_ROOM;
  }

  function resetPanelSpace(): void {
    panelUp = false;
  }

  function close(): void {
    dispatch('close');
  }

  function onToggleEnabled(e: Event): void {
    setOverrideEnabled(deviceId, (e.target as HTMLInputElement).checked);
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') close();
  }
</script>

<svelte:window on:keydown={onKeydown} />

<div class="overlay dp-overlay show" transition:fade={{ duration: 160 }}>
  <div class="dp-backdrop" role="presentation" on:click={close} />

  <div
    class="modal dp-modal"
    class:expanded
    role="dialog"
    aria-modal="true"
    aria-label={device ? $t('device.overrideTitle', { alias: device.alias }) : $t('device.params')}
  >
    <header class="dp-head">
      <span class="dp-title">
        <SlidersHorizontal />
        <span class="dp-title-text">{$t('device.overrideTitle', { alias: device?.alias ?? '' })}</span>
      </span>

      <span class="dp-actions">
        <label class="switch dp-switch" use:tip={$t('device.overrideTip')}>
          <input type="checkbox" checked={enabled} on:change={onToggleEnabled} />
          <span class="slider"></span>
        </label>

        <span
          class="tip-host"
          use:tip={expanded ? $t('device.params.collapse') : $t('device.params.expand')}
        >
          <button
            class="dp-icon-btn"
            aria-label={expanded ? $t('device.params.collapse') : $t('device.params.expand')}
            aria-pressed={expanded}
            on:click={() => (expanded = !expanded)}
          >
            {#if expanded}<Minimize2 />{:else}<Maximize2 />{/if}
          </button>
        </span>

        <button class="dp-icon-btn" aria-label={$t('about.close')} on:click={close}><X /></button>
      </span>
    </header>

    <!-- 取值已保留必须有文字说明：只靠字段变灰既可读作「已禁用」也可读作「已清空」（design D8）。 -->
    {#if !enabled && keptCount > 0}
      <div class="dp-note">{$t('device.params.disabledNote')}</div>
    {/if}

    <div
      class="dp-body"
      class:panel-up={panelUp}
      on:pointerover={trackPanelSpace}
      on:focusin={trackPanelSpace}
      on:pointerleave={resetPanelSpace}
      on:focusout={resetPanelSpace}
    >
      <div class="dp-grid" class:ov-off={!enabled}>
        {#each sections as section (section.titleKey)}
          <section class="dp-card" class:wide={section.span}>
            <h3>{$t(section.titleKey)}</h3>
            {#each section.fields as item (item.ref.group + '.' + item.ref.key)}
              <ParamField
                field={item.field}
                value={displayOf(item.ref)}
                error={fieldError(item.ref)}
                on:change={(e) => setOverrideValue(deviceId, item.ref.group, item.ref.key, e.detail)}
              />
            {/each}
          </section>
        {/each}
      </div>
    </div>

    <footer class="dp-foot">
      <span class="dp-foot-label" aria-hidden="true"><Terminal /></span>
      {#if parts}
        <div class="dp-preview" bind:this={previewEl} on:wheel|nonpassive={onPreviewWheel}>
          {parts.bin} -s <span class="dev">{parts.serial}</span>
          {#each parts.args as a}<b>{a}</b>{' '}{/each}
        </div>
      {:else}
        <div class="dp-preview empty">{$t('launchbar.noCommand')}</div>
      {/if}
      {#if errorTexts.length > 0}
        <span class="dp-warn" use:tip={errorTexts.join('\n')}>
          <AlertTriangle /> {$t('launchbar.invalidCount', { count: errorTexts.length })}
        </span>
      {/if}
    </footer>
  </div>
</div>
