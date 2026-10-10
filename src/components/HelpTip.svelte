<script lang="ts">
  // 「按需展开的说明」的结构件（design D1）：只包 `.fhelp` / `.fhelp-btn` / `.fhelp-panel` 三件。
  // 必须让面板是 `.fhelp` 的**后代**（不能挪到外面）—— `:hover` 与 `:focus-within` 都要靠这层
  // 包含关系才能把「指针移进面板」与「焦点还在里面」算作仍然展开。
  //
  // 展开**全由 CSS 承担**，所以这里没有 store、没有事件、没有 `aria-expanded`：
  //   `.fhelp:hover`        → 悬浮展开（移开即收起）
  //   `.fhelp:focus-within`→ 键盘聚焦展开；鼠标点击也会让按钮获得焦点，于是「点击后保持展开」
  //                           零成本成立，而「同一时刻最多一块被固定」由焦点唯一性自然保证。
  // 代价（用户 2026-10-08 接受）：不能「再点一下收起」—— 收起要点页面别处，或用键盘把焦点移走。
  //
  // 面板内容用插槽给，且**每段都要自己带上 `.fhelp-p`**（块级 + 段间距规则在 app.css）：
  // 这样调用处与 `ui-design/launcher-prototype.html` 的标记逐段对得上，也容得下段内的动态内容
  // （如「目录」那段里由后端下发的约定目录文字）。
  import { CircleHelp } from 'lucide-svelte';

  /** 面板顶部的标题，与字段标签同名（如「更新」）。 */
  export let title: string;
  /** 入口的无障碍名称（如「更新的说明」）—— 入口是只有图标的按钮，必须自带名称。 */
  export let label: string;
</script>

<span class="fhelp">
  <button class="fhelp-btn" type="button" aria-label={label}>
    <CircleHelp />
  </button>
  <span class="fhelp-panel"><span class="fhelp-title">{title}</span><slot /></span>
</span>
