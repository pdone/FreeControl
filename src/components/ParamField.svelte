<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { ParamField as PF } from '../lib/params';
  import FieldLabel from './FieldLabel.svelte';

  export let field: PF;
  export let value: unknown;
  /** 校验失败的提示（tasks 7.3）；为空表示该字段合法（或未填写）。 */
  export let error: string | null = null;

  const dispatch = createEventDispatcher<{ change: unknown }>();
  const set = (v: unknown) => dispatch('change', v);

  // 滑块「已填充」比例：由取值相对量程计算并写入 CSS 变量 --p（design D10）。
  // 走响应式计算而非依赖 input 事件，保证程序化赋值（不触发 input）后填充段也会重算。
  $: pct =
    field.type === 'range' &&
    typeof field.min === 'number' &&
    typeof field.max === 'number' &&
    field.max > field.min
      ? ((Number(value) - field.min) / (field.max - field.min)) * 100
      : 0;
  $: rangeStyle = `--p:${Math.max(0, Math.min(100, pct))}%`;

  function rangeDisplay(v: unknown): string {
    if (field.autoZero && (v === 0 || v === '' || v === null || v === undefined)) {
      return field.autoZero;
    }
    if (field.unit) return `${v} ${field.unit}`;
    return `${v}`;
  }

  // **模板里不要直接调用读取 `value` 的函数**：Svelte 的依赖跟踪只能看到表达式里
  // 直接出现的变量，看不到函数体内部的 `value`，于是 `{rangeDisplay()}` 这种写法
  // 会永远显示首次渲染的旧值（拖动滑块时数字回显不动、值被程序化改掉后输入框不回填）。
  // 统一改成「响应式变量 + 模板只引用该变量」。
  $: rangeText = rangeDisplay(value);
  $: textValue = value === null || value === undefined ? '' : String(value);
  $: activeChips = new Set(Array.isArray(value) ? (value as string[]) : []);

  function toggleChip(opt: string) {
    const arr: string[] = Array.isArray(value) ? [...(value as string[])] : [];
    const i = arr.indexOf(opt);
    if (i >= 0) arr.splice(i, 1);
    else arr.push(opt);
    set(arr);
  }

  // 事件处理：Svelte 4 不支持在模板表达式里写 TS，故强制转换放在 <script> 内。
  function onToggle(e: Event) {
    set((e.target as HTMLInputElement).checked);
  }
  function onSelect(e: Event) {
    set((e.target as HTMLSelectElement).value);
  }
  function onRange(e: Event) {
    set(Number((e.target as HTMLInputElement).value));
  }
  function onText(e: Event) {
    const el = e.target as HTMLInputElement;
    set(field.type === 'number' ? (el.value === '' ? '' : Number(el.value)) : el.value);
  }
  function onTextarea(e: Event) {
    set((e.target as HTMLTextAreaElement).value);
  }
</script>

{#if field.type === 'toggle'}
  <div class="field">
    <FieldLabel {field} />
    <div class="toggle">
      <span class="t-label">{error ?? field.hint ?? ''}</span>
      <label class="switch">
        <input type="checkbox" checked={Boolean(value)} on:change={onToggle} />
        <span class="slider"></span>
      </label>
    </div>
    {#if error}<div class="hint dl-error">{error}</div>{/if}
  </div>
{:else if field.type === 'segmented'}
  <div class="field">
    <FieldLabel {field} />
    <div class="seg">
      {#each field.options ?? [] as o}
        <button class:active={value === o.value} on:click={() => set(o.value)}>{o.label}</button>
      {/each}
    </div>
    {#if error}
      <div class="hint dl-error">{error}</div>
    {:else if field.hint}
      <div class="hint">{field.hint}</div>
    {/if}
  </div>
{:else if field.type === 'select'}
  <div class="field">
    <FieldLabel {field} />
    <select on:change={onSelect}>
      {#each field.options ?? [] as o}
        <option value={o.value} selected={value === o.value}>{o.label}</option>
      {/each}
    </select>
    {#if error}
      <div class="hint dl-error">{error}</div>
    {:else if field.hint}
      <div class="hint">{field.hint}</div>
    {/if}
  </div>
{:else if field.type === 'chips'}
  <div class="field">
    <FieldLabel {field} />
    <div class="chips">
      {#each field.options ?? [] as o}
        <span
          class="chip"
          class:active={activeChips.has(o.value)}
          on:click={() => toggleChip(o.value)}
          on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && toggleChip(o.value)}
          role="button"
          tabindex="0">{o.label}</span
        >
      {/each}
    </div>
    {#if error}
      <div class="hint dl-error">{error}</div>
    {/if}
  </div>
{:else if field.type === 'range'}
  <div class="field">
    <FieldLabel {field} />
    <div class="range-row">
      <input
        type="range"
        min={field.min}
        max={field.max}
        step={field.step}
        value={Number(value) || 0}
        style={rangeStyle}
        aria-invalid={error ? 'true' : 'false'}
        on:input={onRange}
      />
      <span class="range-val">{rangeText}</span>
    </div>
    {#if error}
      <div class="hint dl-error">{error}</div>
    {:else if field.hint}
      <div class="hint">{field.hint}</div>
    {/if}
  </div>
{:else if field.type === 'textarea'}
  <div class="field">
    <FieldLabel {field} />
    <textarea
      rows="3"
      placeholder={field.placeholder}
      value={textValue}
      aria-invalid={error ? 'true' : 'false'}
      on:input={onTextarea}
    ></textarea>
    {#if error}
      <div class="hint dl-error">{error}</div>
    {:else if field.hint}
      <div class="hint">{field.hint}</div>
    {/if}
  </div>
{:else}
  <!-- text / number -->
  <div class="field">
    <FieldLabel {field} />
    <input
      type={field.type === 'number' ? 'number' : 'text'}
      min={field.min}
      max={field.max}
      step={field.step}
      placeholder={field.placeholder}
      value={textValue}
      aria-invalid={error ? 'true' : 'false'}
      on:input={onText}
    />
    {#if error}
      <div class="hint dl-error">{error}</div>
    {:else if field.hint}
      <div class="hint">{field.hint}</div>
    {/if}
  </div>
{/if}
