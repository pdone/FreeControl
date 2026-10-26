<script lang="ts">
  // 图标取自入库的 lucide 子集（`icon-set.ts`，由 scripts/gen-icon-subset.mjs 生成，
  // 与产品 package.json 的 lucide-svelte 同版）：按名从 `ICONS` 取图形节点，序列化成 <svg> 渲染。
  // 刻意不用 `lucide.createIcons()` 那种「就地替换 DOM」的做法 ——
  // 那是给静态页用的，会与 Svelte 的视图所有权打架（切换语言重建节点时对不上）。
  // 取不到图标时优雅降级为不渲染（图标为空，不影响其余内容）。
  import { ICONS } from './icon-set';

  export let name: string; // lucide 的 PascalCase 名，如 "Github"、"Download"
  export let size = 16;
  export let strokeWidth = 2;

  // lucide 的图形节点形如 ['svg', attrs, [子节点…]]，子节点同构（如 ['path', {d}]）。
  type IconChild = [string, Record<string, string | number>, IconChild[]?];

  const esc = (v: string): string =>
    v.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

  const attrsOf = (attrs: Record<string, string | number>): string =>
    Object.entries(attrs)
      .map(([k, v]) => `${k}="${esc(String(v))}"`)
      .join(' ');

  function renderChild([tag, attrs, children]: IconChild): string {
    const inner = children?.length ? children.map(renderChild).join('') : '';
    return `<${tag} ${attrsOf(attrs)}>${inner}</${tag}>`;
  }

  function render(node: IconChild | undefined, s: number, sw: number): string {
    if (!node) return '';
    const children = (node[2] ?? []).map(renderChild).join('');
    return `<svg class="ico" xmlns="http://www.w3.org/2000/svg" width="${s}" height="${s}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="${sw}" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${children}</svg>`;
  }

  let warned = false;
  $: node = ICONS[name] as IconChild | undefined;
  $: if (!node && !warned) {
    warned = true;
    console.warn(`[Icon] 未知的 lucide 图标：${name}`);
  }
  $: markup = render(node, size, strokeWidth);
</script>

{@html markup}
