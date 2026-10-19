<script lang="ts">
  // 字段标签行（`.flabel`）：标签正文 + 可选的「?」详情入口。
  //
  // 说明分两层（config.yaml ④）：字段下方只留一句**短常驻说明**（`hint`），
  // 较长、或带平台限制 / 前置条件的文案按语义拆段收进 `?` 里的详情（`detail`）。
  // 面板必须留在 `.flabel` **内部** —— `.field > .flabel` 是它的定位祖先，
  // 展开（`:hover` / `:focus-within`）也靠这层包含关系（见 HelpTip 的注释）。
  import HelpTip from './HelpTip.svelte';
  import type { ParamField } from '../lib/params';

  export let field: ParamField;
</script>

<span class="flabel"><code>{field.label}</code>{#if field.detail?.length}<HelpTip
      title={field.label}
      label={`${field.label}的说明`}
    >{#each field.detail as p}<span class="fhelp-p">{p}</span>{/each}</HelpTip>{/if}</span>
