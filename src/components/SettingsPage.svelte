<script lang="ts">
  // 「设置」页（spec `app-preferences`）：本程序自身的外观偏好，与 scrcpy 参数无关。
  // 页键 `settings`、入口名「设置」、组件文件名三处一致（页面身份三件套）。
  //
  // 两个字段的写入都走 store 的 setThemeMode / setAccent（改 `config.ui` + 去抖落盘），
  // 视觉由 theme.ts 的订阅立即应用 —— 本页不自己写 DOM 样式，也不碰 `defaults.*`。
  // 语言选择不在本次范围内（后续单独提案），故本页只有「外观」一张卡。
  import { config, setAccent, setThemeMode } from '../lib/store';
  import {
    ACCENTS,
    DEFAULT_ACCENT,
    DEFAULT_THEME,
    inkFor,
    normalizeHex,
    type ThemeMode,
  } from '../lib/theme';
  import HelpTip from './HelpTip.svelte';
  import { Check } from 'lucide-svelte';

  const THEMES: Array<{ value: ThemeMode; label: string }> = [
    { value: 'system', label: '跟随系统' },
    { value: 'dark', label: '深色' },
    { value: 'light', label: '浅色' },
  ];

  $: theme = ($config?.ui?.theme ?? DEFAULT_THEME) as ThemeMode;
  // 配置里的主题色已是合法 #rrggbb（后端归一化过）；这里再小写一次只为让「选中的预设」
  // 与色块取值逐字相等，不引入第二套判定。
  $: accent = ($config?.ui?.accent ?? DEFAULT_ACCENT).toLowerCase();
  // 当前主题色不与任何预设重合 ⇒ 算「自定义」，自定义色块显示选中标记。
  $: isCustom = !ACCENTS.some((a) => a.value === accent);

  function pickPreset(value: string): void {
    setAccent(value);
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
  <h2>设置</h2>
  <p>管理本程序自身的外观：深浅色主题与主题色。改动即时生效并自动保存，不需要重启。</p>
</div>

<div class="grid">
  <div class="card col2">
    <h3>外观</h3>
    <div class="field">
      <span class="flabel">主题模式<HelpTip title="主题模式" label="主题模式的说明">
          <span class="fhelp-p"
            >出厂默认跟随系统：取系统深浅色，运行期间随系统变化；选「深色」或「浅色」则固定，不再跟随系统。</span
          >
        </HelpTip></span
      >
      <div class="seg">
        {#each THEMES as t (t.value)}
          <button class:active={theme === t.value} on:click={() => setThemeMode(t.value)}
            >{t.label}</button
          >
        {/each}
      </div>
    </div>

    <div class="field">
      <span class="flabel">主题色<HelpTip title="主题色" label="主题色的说明">
          <span class="fhelp-p"
            >预设经过挑选，自定义主题色以可读性优先；点「自定义主题色」用系统或内核取色器取色。</span
          >
          <span class="fhelp-p">实底文字自动取白或黑，高亮与浅色文字按当前主题派生。</span>
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
            aria-label={a.label}
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
          <span>自定义主题色</span>
          <span class="color-swatch-wrap" class:sel={isCustom} style:color={inkFor(accent)}>
            <input type="color" value={accent} on:change={applyCustom} aria-label="自定义主题色" />
            {#if isCustom}<Check />{/if}
          </span>
        </label>
        <code class="accent-hex">{accent}</code>
      </div>
    </div>
  </div>
</div>
