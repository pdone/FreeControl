// 访客平台判定（纯函数，不读 window，便于用 node 直跑核对）。
//
// 「下载」一节的主按钮取哪一档产物、平台卡标不标「你的系统」，都读这一处结果 ——
// 同一节里 MUST NOT 各判一次（spec `project-website`）。
//
// 架构判定宁可保守：只有 UA-CH 的 `architectures` 明确含 arm、或 UA 字符串明确含
// `ARM64` / `aarch64` 时才判 arm64，否则一律回落 x64。理由是反向不兼容 ——
// x64 产物在 ARM64 Windows 上能经系统仿真运行，而 arm64 产物在 x64 上根本起不来。

export type Os = 'windows' | 'linux' | 'other';
export type Arch = 'x64' | 'arm64';

export interface PlatformHints {
  userAgent: string;
  /** `navigator.userAgentData?.architectures`（UA-CH）；Firefox / Safari 没有，传空即可。 */
  architectures?: readonly string[] | null;
}

export function detectPlatform(input: PlatformHints): { os: Os; arch: Arch } {
  const ua = input.userAgent ?? '';
  const os: Os = /Android/i.test(ua)
    ? 'other'
    : /Windows/i.test(ua)
      ? 'windows'
      : /Linux|X11/i.test(ua)
        ? 'linux'
        : 'other';

  const armByUaCh = (input.architectures ?? []).some((a) => /arm/i.test(a));
  const armByUa = /ARM64|aarch64/i.test(ua);
  return { os, arch: armByUaCh || armByUa ? 'arm64' : 'x64' };
}
