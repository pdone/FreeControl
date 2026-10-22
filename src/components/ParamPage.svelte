<script lang="ts">
  import { defaults, defaultsErrors, config, scheduleSave } from '../lib/store';
  import { t } from '../lib/i18n';
  import { PARAM_GROUPS, refKey } from '../lib/params';
  import ParamField from './ParamField.svelte';

  export let groupKey: string;
  $: group = PARAM_GROUPS.find((g) => g.key === groupKey);

  // 本地克隆该组的默认参数用于绑定；变更后写回 config.defaults 并去抖落盘。
  type GroupValues = Record<string, unknown>;
  type GroupMap = Record<string, GroupValues>;

  let vals: GroupValues = {};
  $: {
    const d = $defaults as unknown as GroupMap;
    if (d && d[groupKey]) vals = { ...d[groupKey] };
  }

  function update(key: string, v: unknown) {
    vals = { ...vals, [key]: v };
    config.update((c) => {
      if (!c) return c;
      const cur = (c.defaults as unknown as GroupMap) ?? {};
      const nextDefaults = { ...cur, [groupKey]: { ...vals } } as unknown as typeof c.defaults;
      return { ...c, defaults: nextDefaults };
    });
    scheduleSave();
  }
</script>

{#if group}
  <div class="sec-head">
    <h2>{$t(group.titleKey)}</h2>
    <p>{$t(group.subtitleKey)}</p>
  </div>
  <div class="grid">
    {#each group.cards as card}
      {#if !card.showWhen || card.showWhen(vals)}
        <div class="card" class:col2={card.span2}>
          <h3>{$t(card.titleKey)}</h3>
          {#each card.fields as f}
            {#if !f.showWhen || f.showWhen(vals)}
              <ParamField
                field={f}
                value={vals[f.key]}
                error={$defaultsErrors[refKey(groupKey, f.key)] ?? null}
                on:change={(e) => update(f.key, e.detail)}
              />
            {/if}
          {/each}
        </div>
      {/if}
    {/each}
  </div>
{/if}
