<script lang="ts">
  // 字段标签 + 可选的「?」详情（design D1）。文案键在渲染时解析，flag 名保持原文。
  import type { ParamField } from '../lib/params';
  import { t } from '../lib/i18n';
  import { labelText } from '../lib/paramText';
  import HelpTip from './HelpTip.svelte';

  export let field: ParamField;

  $: label = labelText(field, $t);
  $: detailKeys = field.detailKeys ?? [];
</script>

<span class="flabel"><code>{label}</code>{#if detailKeys.length}<HelpTip
      title={label}
      label={$t('common.fieldHelp', { field: label })}
    >{#each detailKeys as key}<span class="fhelp-p">{$t(key)}</span>{/each}</HelpTip>{/if}</span>
