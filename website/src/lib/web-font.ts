/**
 * 官网正文字体：阿里巴巴普惠体 3.0（55 Regular + 75 SemiBold）子集，**懒加载**。
 *
 * 为什么是懒加载而不是 `@font-face` + `preload`：字体是这个站锦上添花的一层（首屏用系统栈
 * 本来就完整可读），而任何让字体挤进首屏关键路径的做法都会拖慢 h1 与 hero 预览的绘制。
 * 这里的顺序是：系统栈先把字画出来 → `load` 之后的空闲帧再取两个 woff2 → 两份都到位后
 * 才把 `--font-sans` 切过去。于是没有 FOUT（文字不会先隐形/闪系统体），只有一次"字体到位"
 * 的整体替换；取不到就一直是系统栈，功能与可读性都不受影响。
 *
 * 为什么字体只做了子集：普惠体全量 TTF 8 MB 一个字重。子集由 `scripts/gen-font-subset.mjs`
 * 按页面实际用字生成（721 字 → 每个字重 76.5 KB），是入库的生成物；改文案后若出现子集外的字，
 * 浏览器按字回退到系统栈，不会出现豆腐块。
 *
 * 字重映射：SemiBold 一份以 `weight: '600 900'` 注册。站内标题要 600–800，把它们全部落到
 * 这一份轮廓上 —— 否则浏览器会给 Regular 描粗（合成粗体），h1 的笔画会"糊"且比真粗体轻。
 * 代价是 700/800 与 600 同形（视觉上等于"标题统一用 SemiBold"），这正是本站要的：
 * 层级靠字号与间距，不靠 800 与 700 的差别。
 * 若日后要更重的标题，补一份 85 Bold 的子集、在这里加第三个 FontFace 即可，CSS 不用改。
 *
 * 两档一起成功才切换（`Promise.all`）：只有一档到位时，"正文普惠体 + 标题雅黑"比全用系统栈更难看。
 */
const FAMILY = 'Alibaba PuHuiTi 3';
const CLASS = 'has-web-font';

/** 与生成器 `scripts/gen-font-subset.mjs` 的产物名一一对应，改一处要改两处。 */
const FACES: Array<{ file: string; weight: string }> = [
  { file: 'puhuiti3-55-subset.woff2', weight: '400' },
  { file: 'puhuiti3-75-subset.woff2', weight: '600 900' },
];

/** 省流模式：多一份字体不值得，直接用系统栈（注入预览时同样跳过）。 */
function saveData(): boolean {
  return Boolean((navigator as unknown as { connection?: { saveData?: boolean } }).connection?.saveData);
}

function faceUrl(file: string): string {
  return new URL(`../assets/fonts/${file}`, import.meta.url).href;
}

/**
 * 两档 `@font-face` 的 CSS 文本，供宿主注入到 hero 的预览文档里（见 App.svelte 的 injectPreviewFont）。
 * 为什么用注入而不是让原型自己带字体：原型必须自足（config ㊲ / spec「hero 内嵌原型自足」），
 * 不能引 @font-face 或任何外部请求；而站点已经为正文取过这两份 woff2，注入只是复述它们的 URL，
 * 同源同文件 ⇒ 命中同一份 HTTP 缓存，不产生第二次下载。
 */
export function webFontFacesCss(): string {
  if (typeof FontFace === 'undefined' || saveData()) return '';
  return FACES.map(
    (spec) =>
      `@font-face{font-family:'${FAMILY}';src:url('${faceUrl(spec.file)}') format('woff2');` +
      `font-weight:${spec.weight};font-style:normal;font-display:swap}`,
  ).join('\n');
}

export function loadWebFont(): void {
  if (typeof FontFace === 'undefined' || document.documentElement.classList.contains(CLASS)) return;
  if (saveData()) return;

  const faces: FontFace[] = [];
  try {
    for (const spec of FACES) {
      faces.push(new FontFace(FAMILY, `url(${faceUrl(spec.file)}) format('woff2')`, { weight: spec.weight, style: 'normal' }));
    }
  } catch {
    return; // 老引擎不认 FontFace：保持系统栈
  }
  for (const f of faces) document.fonts.add(f);
  Promise.all(faces.map((f) => f.load()))
    .then(() => document.documentElement.classList.add(CLASS))
    .catch(() => {
      /* 任一档取不到（离线、被拦截、子集损坏）就整体不切换：不抛错、不提示 */
    });
}
