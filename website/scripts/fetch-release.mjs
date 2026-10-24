// 构建期解析本仓库的「最新发行」，生成站点下载直链，写进 src/lib/release.json（.gitignore 排除）。
// 目的：下载入口既能一键直下具体产物，又不在源码里写死版本字面量 —— 版本只有一处来源（GitHub 发行）。
// 取不到时（离线 / 限流 / 仓库未发布）写一份 ok:false，站点回退到发行页，构建照常成功。
// 硬性契约：与 sync-assets.mjs 一样，是构建前必跑的步骤（build / dev 脚本已并入）。
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = 'pdone/FreeControl2';
const siteRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const OUT = path.join(siteRoot, 'src', 'lib', 'release.json');
const PAGE = `https://github.com/${REPO}/releases/latest`;

const write = (data) => fs.writeFileSync(OUT, `${JSON.stringify(data, null, 2)}\n`);

// 首选：发行 API。直接拿 GITHUB 给出的 browser_download_url（与产物名解耦，改名也不受影响）。
async function viaApi() {
  const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
    headers: { accept: 'application/vnd.github+json', 'user-agent': 'freecontrol2-website-build' },
  });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  const rel = await res.json();
  const pick = (re) => rel.assets?.find((a) => re.test(a.name))?.browser_download_url ?? null;
  return {
    tag: rel.tag_name,
    // 产物名自 v1.2.0 起统一为 `{产品名}_{版本}_{平台}_{架构}`。exe 按架构各一份（`_x64` /
    // `_arm64`），不能只按 `.exe` 后缀取第一份；这两条与 AppImage / deb 的匹配都以 `$` 结尾，
    // 加了平台段照样命中，不必跟着改。只有 macOS 的架构段由 `aarch64` 改成了 `arm64`，
    // 那一条必须换 —— 否则 Apple 芯片那份永远取不到。
    windows: pick(/_x64\.exe$/i),
    windowsArm64: pick(/_arm64\.exe$/i),
    appimage: pick(/\.AppImage$/),
    deb: pick(/\.deb$/),
    macosArm64: pick(/_macos_arm64\.dmg$/i),
    macosX64: pick(/_macos_x64\.dmg$/i),
  };
}

// 兜底：不占用 API 限额，从 /releases/latest 的 302 读出 tag，再按工作流的产物命名拼直链
// （见 .github/workflows/build.yml：`FreeControl2_<v>_windows_{x64|arm64}.exe`、
// `_linux_amd64.{AppImage|deb}`、`_macos_{arm64|x64}.dmg`）。这条路径没有资产清单可比对，
// 名字拼错只能靠 HEAD 发现，故六条全名与构建侧的改名步骤逐字对齐。
// dmg 早于 macOS 档位的发行里没有，拼出的链接会 404 —— 置空让它单独回退到发行页，不拖垮整页。
async function assetExists(url) {
  try {
    const res = await fetch(url, { method: 'HEAD', redirect: 'manual' });
    return res.ok || (res.status >= 300 && res.status < 400);
  } catch {
    return false;
  }
}

async function viaRedirect() {
  const res = await fetch(PAGE, { redirect: 'manual' });
  const tag = (res.headers.get('location') || '').match(/\/releases\/tag\/(.+)$/)?.[1];
  if (!tag) throw new Error('重定向里没有 tag');
  const v = tag.replace(/^v/, '');
  const base = `https://github.com/${REPO}/releases/download/${tag}`;
  const macosArm64 = `${base}/FreeControl2_${v}_macos_arm64.dmg`;
  const macosX64 = `${base}/FreeControl2_${v}_macos_x64.dmg`;
  const [hasArm, hasIntel] = await Promise.all([assetExists(macosArm64), assetExists(macosX64)]);
  return {
    tag,
    windows: `${base}/FreeControl2_${v}_windows_x64.exe`,
    windowsArm64: `${base}/FreeControl2_${v}_windows_arm64.exe`,
    appimage: `${base}/FreeControl2_${v}_linux_amd64.AppImage`,
    deb: `${base}/FreeControl2_${v}_linux_amd64.deb`,
    macosArm64: hasArm ? macosArm64 : null,
    macosX64: hasIntel ? macosX64 : null,
  };
}

let resolved = null;
for (const [name, fn] of [
  ['发行 API', viaApi],
  ['重定向兜底', viaRedirect],
]) {
  try {
    resolved = await fn();
    console.log(`[fetch-release] 经「${name}」解析到 ${resolved.tag}`);
    break;
  } catch (err) {
    console.warn(`[fetch-release]「${name}」未成功：${err.message}`);
  }
}

// 四档核心产物齐备才算完整：与构建侧「缺任一份即失败」同口径，避免半套发行被当成可用版本
// （少一份时整页回退发行页，而不是给出一个指向 404 的直链）。
// macOS 的两份 dmg 不纳入 `ok`：老发行没有它们时，只让 macOS 那两颗入口回退到发行页。
const ok = !!(resolved && resolved.windows && resolved.windowsArm64 && resolved.appimage && resolved.deb);
if (ok) {
  write({
    ok: true,
    tag: resolved.tag,
    windows: resolved.windows,
    windowsArm64: resolved.windowsArm64,
    appimage: resolved.appimage,
    deb: resolved.deb,
    macosArm64: resolved.macosArm64 ?? null,
    macosX64: resolved.macosX64 ?? null,
    page: PAGE,
  });
  console.log('[fetch-release] 直链已写入 src/lib/release.json');
} else {
  write({
    ok: false,
    tag: null,
    windows: null,
    windowsArm64: null,
    appimage: null,
    deb: null,
    macosArm64: null,
    macosX64: null,
    page: PAGE,
  });
  console.warn('[fetch-release] 未取到完整产物，回退到发行页（站点构建照常）');
}
