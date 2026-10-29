/**
 * 官网正文字体：阿里巴巴普惠体 3.0，只带 55 Regular 一份子集。
 *
 * 声明时机：在首帧之前（见 `main.ts` 的 `installWebFont()`）。站点是纯客户端渲染，正文第一个
 * 字符必然出现在 JS 执行之后，所以这里同步插进 `document.head` 的 `@font-face` 一定先于任何
 * 文字绘制生效 —— 字体已在浏览器缓存里时，首帧直接就是普惠体，不再有"先系统字体、再整体换
 * 一次"的过程（那正是以前挂在 `load` 之后懒加载的代价）。
 *
 * 本地优先：这条 `@font-face` 用 `'Alibaba PuHuiTi 3'` 这个族名，而 `tokens.css` 的字体栈把
 * 本机安装族名（`"Alibaba PuHuiTi 3.0"` 等）排在它前面。两者必须不同名：同族名的 `@font-face`
 * 会覆盖本机字体，本地那份就永远用不到。本机装了普惠体时首个族名即命中，浏览器不会来取这份
 * woff2（整页零字体请求）；没装才由它接管。
 *
 * 为什么字体只做了子集：普惠体全量 TTF 一份字重 8 MB。子集由 `scripts/gen-font-subset.mjs`
 * 按页面实际用字生成（721 字 → 76.5 KB），是入库的生成物；改文案后若出现子集外的字，浏览器
 * 按字回退到系统栈，不会出现豆腐块。
 *
 * 为什么只有 400 这一档：见 spec「正文字体只提供单一字重档」。标题要 600–900，浏览器会在这
 * 一份轮廓上合成加粗 —— 不再另带 75 SemiBold，冷缓存时要下的字节也只有一半。
 *
 * 省流模式：不声明 `@font-face`，也就不发请求（栈里那个族名没有对应字体，浏览器直接跳到系统栈）。
 */
const FAMILY = 'Alibaba PuHuiTi 3';
/** 与生成器 `scripts/gen-font-subset.mjs` 的产物名一一对应，改一处要改两处。 */
const FILE = 'puhuiti3-55-subset.woff2';
const STYLE_ID = 'fc-web-font';

/** 省流模式：多一份字体不值得，直接用系统栈（注入预览时同样跳过）。 */
function saveData(): boolean {
  return Boolean((navigator as unknown as { connection?: { saveData?: boolean } }).connection?.saveData);
}

function faceUrl(file: string): string {
  return new URL(`../assets/fonts/${file}`, import.meta.url).href;
}

/**
 * 那条 `@font-face` 的 CSS 文本，也供宿主注入到 hero 的预览文档里（见 App.svelte 的
 * injectPreviewFont）。为什么用注入而不是让原型自己带字体：原型必须自足（config ㊲ / spec
 * 「hero 内嵌原型自足、不请求外部资源」），不能引 @font-face 或任何外部请求；而站点已经为
 * 正文声明过这份 woff2，注入只是复述它的 URL，同源同文件 ⇒ 命中同一份 HTTP 缓存，不产生第
 * 二次下载。省流时返回空串，调用方据此跳过注入。
 */
export function webFontFacesCss(): string {
  if (saveData()) return '';
  return (
    `@font-face{font-family:'${FAMILY}';src:url('${faceUrl(FILE)}') format('woff2');` +
    `font-weight:400;font-style:normal;font-display:swap}`
  );
}

/** 在首帧之前声明正文字体；重复调用无副作用（省流或已声明时直接返回）。 */
export function installWebFont(): void {
  if (typeof document === 'undefined' || document.getElementById(STYLE_ID)) return;
  const css = webFontFacesCss();
  if (!css) return;
  const style = document.createElement('style');
  style.id = STYLE_ID;
  style.textContent = css;
  document.head.appendChild(style);
}
