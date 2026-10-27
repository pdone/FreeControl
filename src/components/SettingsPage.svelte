<script lang="ts">
  // 「设置」页（spec `app-preferences`）：本程序自身的外观、更新与日志内容。
  // 页键 `settings`、入口名「设置」、组件文件名三处一致（页面身份三件套）。
  //
  // 外观两个字段走 setThemeMode / setAccent（改 `config.ui`），视觉由 theme.ts 的订阅立即应用；
  // 自动更新走 setAutoUpdate / setUpdateFrequency（改 `config.update`），上次检查时间由检查包装写入。
  // 两者都复用既有去抖保存，不碰 `defaults.*`。页内两张偏好卡同排等宽：外观 + 自动更新。
  // 日志内容由 `SettingsLogCards` 渲染：运行日志参数 / 日志文件同排在前，会话输出独占整行放在页面底部。
  // 界面语言是本次新增的第三个外观字段：改 `config.ui.language`，由 i18n 订阅立即应用。
  import {
    config,
    openUpdateDialog,
    setAccent,
    setAutoUpdate,
    setCodeFont,
    setThemeMode,
    setUiFont,
    setUiLanguage,
    setUiScale,
    setUpdateFrequency,
  } from '../lib/store';
  import {
    ACCENTS,
    DEFAULT_ACCENT,
    DEFAULT_THEME,
    inkFor,
    normalizeHex,
    type ThemeMode,
  } from '../lib/theme';
  import { DEFAULT_UI_SCALE, SCALES } from '../lib/scale';
  import HelpTip from './HelpTip.svelte';
  import FontPicker from './FontPicker.svelte';
  import SettingsLogCards from './SettingsLogCards.svelte';
  import { Check, RefreshCw } from 'lucide-svelte';
  import { DEFAULT_LANGUAGE, t, type MessageKey } from '../lib/i18n';
  import type { LanguagePref, UpdateFrequency, UpdatePrefs } from '../lib/types';

  const THEMES: Array<{ value: ThemeMode; labelKey: MessageKey }> = [
    { value: 'system', labelKey: 'settings.theme.system' },
    { value: 'dark', labelKey: 'settings.theme.dark' },
    { value: 'light', labelKey: 'settings.theme.light' },
  ];

  const UPDATES: Array<{ value: UpdateFrequency; labelKey: MessageKey }> = [
    { value: 'launch', labelKey: 'settings.update.freq.launch' },
    { value: 'daily', labelKey: 'settings.update.freq.daily' },
    { value: 'weekly', labelKey: 'settings.update.freq.weekly' },
    { value: 'monthly', labelKey: 'settings.update.freq.monthly' },
  ];

  const LANGUAGES: Array<{ value: LanguagePref; labelKey: MessageKey }> = [
    { value: 'system', labelKey: 'settings.language.followSystem' },
    { value: 'zh', labelKey: 'settings.language.zh' },
    { value: 'en', labelKey: 'settings.language.en' },
  ];

  const DEFAULT_UPDATE_PREFS: UpdatePrefs = {
    autoCheck: false,
    frequency: 'launch',
    lastCheckedAt: null,
  };

  $: language = $config?.ui?.language ?? DEFAULT_LANGUAGE;
  $: theme = ($config?.ui?.theme ?? DEFAULT_THEME) as ThemeMode;
  // 配置里的主题色已是合法 #rrggbb（后端归一化过）；这里再小写一次只为让「选中的预设」
  // 与色块取值逐字相等，不引入第二套判定。
  $: accent = ($config?.ui?.accent ?? DEFAULT_ACCENT).toLowerCase();
  // 当前主题色不与任何预设重合 ⇒ 算「自定义」，自定义色块显示选中标记。
  $: isCustom = !ACCENTS.some((a) => a.value === accent);
  // 界面缩放：配置里的取值已由后端归一化过，缺字段时按出厂 100%。
  $: uiScale = $config?.ui?.uiScale ?? DEFAULT_UI_SCALE;

  // 更新检查偏好：与外观同处「设置」，但独立落在 config.update。
  $: updatePrefs = $config?.update ?? DEFAULT_UPDATE_PREFS;
  $: lastChecked = updatePrefs.lastCheckedAt
    ? formatTimestamp(updatePrefs.lastCheckedAt)
    : '';

  function pickPreset(value: string): void {
    setAccent(value);
  }

  function onAutoToggle(e: Event): void {
    setAutoUpdate((e.currentTarget as HTMLInputElement).checked);
  }

  /** 上次检查时间：本地 YYYY-MM-DD HH:mm；从未检查时由模板显示「尚未检查」。 */
  function formatTimestamp(at: number): string {
    const d = new Date(at);
    const pad = (n: number): string => String(n).padStart(2, '0');
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
  }

  /**
   * 自定义取色：控件的值由内核/系统取色器给出，形状本就是 `#rrggbb`。
   * 只在 `change`（确认）时写入 —— 拖动取色器时它自己会预览，不必每一帧都改配置。
   */
  function applyCustom(e: Event): void {
    const hex = normalizeHex((e.currentTarget as HTMLInputElement).value);
    if (hex) setAccent(hex);
  }
</script>

<div class="sec-head">
  <h2>{$t('nav.settings')}</h2>
  <p>{$t('settings.subtitle')}</p>
</div>

<div class="grid">
  <div class="card">
    <h3>{$t('settings.appearance.title')}</h3>
    <div class="field">
      <span class="flabel">{$t('settings.language.label')}<HelpTip title={$t('settings.language.helpTitle')} label={$t('settings.language.helpLabel')}>
          <span class="fhelp-p">{$t('settings.language.help')}</span>
        </HelpTip></span
      >
      <div class="seg">
        {#each LANGUAGES as l (l.value)}
          <button class:active={language === l.value} on:click={() => setUiLanguage(l.value)}
            >{$t(l.labelKey)}</button
          >
        {/each}
      </div>
    </div>

    <div class="field">
      <span class="flabel">{$t('settings.theme.label')}<HelpTip title={$t('settings.theme.helpTitle')} label={$t('settings.theme.helpLabel')}>
          <span class="fhelp-p">{$t('settings.theme.help')}</span>
        </HelpTip></span
      >
      <div class="seg">
        {#each THEMES as m (m.value)}
          <button class:active={theme === m.value} on:click={() => setThemeMode(m.value)}
            >{$t(m.labelKey)}</button
          >
        {/each}
      </div>
    </div>

    <div class="field">
      <span class="flabel">{$t('settings.accent.label')}<HelpTip title={$t('settings.accent.helpTitle')} label={$t('settings.accent.helpLabel')}>
          <span class="fhelp-p">{$t('settings.accent.help1')}</span>
          <span class="fhelp-p">{$t('settings.accent.help2')}</span>
        </HelpTip></span
      >
      <div class="swatches">
        {#each ACCENTS as a (a.value)}
          <button
            type="button"
            class="swatch"
            class:sel={accent === a.value}
            style:background={a.value}
            style:color={inkFor(a.value)}
            aria-label={$t(a.labelKey)}
            aria-pressed={accent === a.value}
            on:click={() => pickPreset(a.value)}
          >
            {#if accent === a.value}<Check />{/if}
          </button>
        {/each}
      </div>
      <div class="color-row">
        <!-- 原生取色控件：点击打开 WebView2/Chromium 的取色器（含色相/明度面板与吸管），
             值本就是 #rrggbb；只在 change（确认）时写入，拖动中由取色器自己预览。
             当前色不是任何预设时，这个色块承担「自定义」的选中标记（外环 + 对号）。 -->
        <label class="color-pick">
          <span>{$t('settings.accent.custom')}</span>
          <span class="color-swatch-wrap" class:sel={isCustom} style:color={inkFor(accent)}>
            <input
              type="color"
              value={accent}
              on:change={applyCustom}
              aria-label={$t('settings.accent.custom')}
            />
            {#if isCustom}<Check />{/if}
          </span>
        </label>
        <code class="accent-hex">{accent}</code>
      </div>
    </div>

    <div class="field">
      <span class="flabel">{$t('settings.uiScale.label')}<HelpTip title={$t('settings.uiScale.helpTitle')} label={$t('settings.uiScale.helpLabel')}>
          <span class="fhelp-p">{$t('settings.uiScale.help')}</span>
        </HelpTip></span
      >
      <div class="seg">
        {#each SCALES as s (s.value)}
          <button
            class:active={uiScale === s.value}
            on:click={() => setUiScale(s.value)}
            >{s.label}</button
          >
        {/each}
      </div>
    </div>
  </div>

  <div class="card">
    <h3>{$t('settings.update.title')}</h3>
    <div class="field">
      <div class="toggle">
        <span class="t-label"
          >{$t('settings.update.autoCheck')}<HelpTip title={$t('settings.update.helpTitle')} label={$t('settings.update.helpLabel')}>
            <span class="fhelp-p">{$t('settings.update.help')}</span>
          </HelpTip></span
        >
        <label class="switch">
          <input type="checkbox" checked={updatePrefs.autoCheck} on:change={onAutoToggle} />
          <span class="slider"></span>
        </label>
      </div>
    </div>
    {#if updatePrefs.autoCheck}
      <div class="field">
        <span class="flabel">{$t('settings.update.frequency')}</span>
        <div class="seg">
          {#each UPDATES as f (f.value)}
            <button
              class:active={updatePrefs.frequency === f.value}
              on:click={() => setUpdateFrequency(f.value)}
              >{$t(f.labelKey)}</button
            >
          {/each}
        </div>
      </div>
    {/if}
    <div class="field">
      <div class="path-row auto-update-status">
        <button class="btn sm" on:click={() => openUpdateDialog()}>
          <RefreshCw />{$t('settings.update.check')}
        </button>
        <span class="hint">
          {updatePrefs.lastCheckedAt
            ? $t('settings.update.lastChecked', { time: lastChecked })
            : $t('settings.update.neverChecked')}
        </span>
      </div>
    </div>
  </div>

  <div class="card col2">
    <h3>{$t('settings.fonts.title')}</h3>
    <div class="field">
      <span class="flabel"
        >{$t('settings.fonts.ui.label')}<HelpTip title={$t('settings.fonts.ui.helpTitle')} label={$t('settings.fonts.ui.helpLabel')}>
          <span class="fhelp-p">{$t('settings.fonts.ui.help')}</span>
        </HelpTip></span
      >
      <FontPicker
        label={$t('settings.fonts.ui.label')}
        value={$config?.ui?.uiFont ?? null}
        on:change={(e) => setUiFont(e.detail)}
      />
      <div class="font-preview">
        <span class="font-preview-label">{$t('settings.fonts.preview')}</span>
        <span style:font-family="var(--font-sans)"
          >{$t('settings.fonts.previewText')}</span
        >
      </div>
    </div>

    <div class="field">
      <span class="flabel"
        >{$t('settings.fonts.code.label')}<HelpTip title={$t('settings.fonts.code.helpTitle')} label={$t('settings.fonts.code.helpLabel')}>
          <span class="fhelp-p">{$t('settings.fonts.code.help')}</span>
        </HelpTip></span
      >
      <FontPicker
        label={$t('settings.fonts.code.label')}
        value={$config?.ui?.codeFont ?? null}
        showMonoBadge
        on:change={(e) => setCodeFont(e.detail)}
      />
      <div class="font-preview">
        <span class="font-preview-label">{$t('settings.fonts.preview')}</span>
        <span style:font-family="var(--font-mono)"
          >{$t('settings.fonts.codePreviewText')}</span
        >
      </div>
    </div>
  </div>

  <SettingsLogCards />
</div>
