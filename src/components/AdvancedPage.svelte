<script lang="ts">
  // 「高级」页（design D2 / D10 / D11）：入口名、页键（`advanced`）与组件文件名三者一致。
  //
  // **页键 ≠ 组键**：本页跨两组渲染 —— 启动应用来自 `apps` 组（`defaults.apps`），
  // 网络与自定义参数来自 `advanced` 组（`defaults.advanced`）。页键 `advanced` 与这两个组键
  // 是两个命名空间：组键是落盘配置的字段名，不随页面改名，别顺手一起改。
  // 卡片按名从参数模型取用（`NETWORK_CARD` / `CUSTOM_ARGS_CARD`），标签、量程与校验错误键
  // 都不在本页重写。
  //
  // 栅格（design D11）：启动应用与网络各占一列、同排；自定义参数整行在其下。是否整行由卡片
  // 自身的 `span2` 决定（与 ParamPage 同一套规则），因此 DOM 顺序必须是
  // 启动应用 → 网络 → 自定义参数 —— 两张单列卡一旦被整行卡隔开就会落到两排。
  //
  // 写回纪律：两组各持一份本地副本，写回时**先合入本地副本再整体写回**。`defaults.advanced`
  // 还会被设置页的日志参数写入，若这里用「只含本卡字段的对象」覆盖整组，会抹掉去抖窗口内
  // 另一页的未落盘改动。
  import { defaults, defaultsErrors, config, scheduleSave } from '../lib/store';
  import { t } from '../lib/i18n';
  import { ADVANCED_GROUP, CUSTOM_ARGS_CARD, NETWORK_CARD, groupOf, refKey } from '../lib/params';
  import ParamField from './ParamField.svelte';

  /**
   * 本组定义。入参 `'apps'` 是**组键**（落盘字段名，不随页键改名）；title 是模型标签「应用」，
   * 与本页页名「高级」有别，故 h2 另行写死为「高级」。
   */
  const APPS_GROUP = groupOf('apps');

  type GroupValues = Record<string, unknown>;
  type GroupMap = Record<string, GroupValues>;

  // ── apps 组（启动应用）──
  let appVals: GroupValues = {};
  $: {
    const d = $defaults as unknown as GroupMap;
    if (d && d.apps) appVals = { ...d.apps };
  }

  function updateApp(key: string, v: unknown) {
    appVals = { ...appVals, [key]: v };
    config.update((c) => {
      if (!c) return c;
      const cur = (c.defaults as unknown as GroupMap) ?? {};
      const nextDefaults = { ...cur, apps: { ...appVals } } as unknown as typeof c.defaults;
      return { ...c, defaults: nextDefaults };
    });
    scheduleSave();
  }

  // ── advanced 组（网络 / 自定义参数）──
  let advVals: GroupValues = {};
  $: {
    const d = $defaults as unknown as GroupMap;
    if (d && d.advanced) advVals = { ...d.advanced };
  }

  function updateAdv(key: string, v: unknown) {
    advVals = { ...advVals, [key]: v };
    config.update((c) => {
      if (!c) return c;
      const cur = (c.defaults as unknown as GroupMap) ?? {};
      const nextDefaults = { ...cur, advanced: { ...advVals } } as unknown as typeof c.defaults;
      return { ...c, defaults: nextDefaults };
    });
    scheduleSave();
  }
</script>

{#if APPS_GROUP}
  <div class="sec-head">
    <h2>{$t('nav.advanced')}</h2>
    <p>{$t(APPS_GROUP.subtitleKey)}</p>
  </div>
  <div class="grid">
    {#each APPS_GROUP.cards as card}
      {#if !card.showWhen || card.showWhen(appVals)}
        <div class="card" class:col2={card.span2}>
          <h3>{$t(card.titleKey)}</h3>
          {#each card.fields as f}
            <ParamField
              field={f}
              value={appVals[f.key]}
              error={$defaultsErrors[refKey(APPS_GROUP.key, f.key)] ?? null}
              on:change={(e) => updateApp(f.key, e.detail)}
            />
          {/each}
        </div>
      {/if}
    {/each}

    <div class="card" class:col2={NETWORK_CARD.span2}>
      <h3>{$t(NETWORK_CARD.titleKey)}</h3>
      {#each NETWORK_CARD.fields as f}
        <ParamField
          field={f}
          value={advVals[f.key]}
          error={$defaultsErrors[refKey(ADVANCED_GROUP.key, f.key)] ?? null}
          on:change={(e) => updateAdv(f.key, e.detail)}
        />
      {/each}
    </div>

    <div class="card" class:col2={CUSTOM_ARGS_CARD.span2}>
      <h3>{$t(CUSTOM_ARGS_CARD.titleKey)}</h3>
      {#each CUSTOM_ARGS_CARD.fields as f}
        <ParamField
          field={f}
          value={advVals[f.key]}
          error={$defaultsErrors[refKey(ADVANCED_GROUP.key, f.key)] ?? null}
          on:change={(e) => updateAdv(f.key, e.detail)}
        />
      {/each}
    </div>
  </div>
{/if}
