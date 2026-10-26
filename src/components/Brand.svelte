<script lang="ts">
  // 品牌标识：使用资产包 desktop/ 分区的成品位图（含品牌方底），与窗口 / 任务栏图标同一份资产，
  // 因此母版的明暗层次在界面里不再被压回平涂。取 128 是为覆盖到 2.5× 缩放档
  // （45 CSS px × 2.5 = 112.5 ≤ 128）；该尺寸由生成流程原生产出，不是放大低位图凑的。
  // 注意：此处为 Vite 构建期导入（产物哈希化到 dist），源码仍只有一份，并非手工副本。
  import logoMark from '../../assets/icons/desktop/128x128.png';
  import { openAboutDialog } from '../lib/store';

  // 品牌行只展示图形标记与产品名 —— 版本信息统一在「关于」弹窗展示（spec「品牌标识」）。
  // 这里原有一条 `$: sub = 'scrcpy ' + $compat.bestFit`：模板里从来没有承载它的元素（纯死代码），
  // 留着会让人以为品牌行还有版本号，故随该要求一并对齐删除（连带不再需要 compat）。

  // 品牌行同时是「关于」弹窗的入口之一（另一个是侧边栏末位的「关于」入口，见 spec `about-page`
  // 「弹窗的打开入口」）：整行可点、键盘可达，收起态只剩图形标记时同样可点。
  // 该交互不改品牌行的呈现与几何 —— 除 CSS 里的指针与一处悬停反馈外不动其它。
  function openAbout(): void {
    openAboutDialog();
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      openAbout();
    }
  }

  // 图片加载失败时移除自身：与 CSS 的 .logo:not(:has(img)) 兜底联动，避免破图。
  // 强制转换放在 <script> 内（Svelte 4 不支持模板表达式写 TS）。
  function hideImg(e: Event) {
    const el = e.target as HTMLImageElement;
    el.style.display = 'none';
  }
</script>

<div
  class="brand clickable"
  role="button"
  tabindex="0"
  on:click={openAbout}
  on:keydown={onKeydown}
>
  <div class="logo">
    <img src={logoMark} alt="FreeControl" on:error={hideImg} />F
  </div>
  <div class="brand-text">
    <div class="name" style="font-size: 16px;">FreeControl</div>

  </div>
</div>
