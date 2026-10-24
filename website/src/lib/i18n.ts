import { derived, writable } from 'svelte/store';

// 「下载」一节平台卡里的一份产物（一行）。
// Windows 同一格式下按架构分两档，故用 `arch` 把架构以徽标呈现；Linux 的区分维度就是格式本身，不给 `arch`。
export interface PlatPackage {
  key: string;
  format: string;
  arch?: string;
  note?: string;
}
export interface Plat {
  os: string;
  name: string;
  packages: PlatPackage[];
  help?: string;
}

// 文案改写自仓库 README.md / README.en.md（同一组章节、一一对应）。
const zh = {
  htmlLang: 'zh-CN',
  metaTitle: 'FreeControl2 — 在 PC 上控制 Android 设备',
  nav: {
    features: '功能特性',
    compare: '对比',
    quickstart: '快速开始',
    faq: '常见问题',
    download: '下载',
    switchLabel: 'EN',
    switchTitle: 'Switch to English',
  },
  hero: {
    tagline: '在 PC 上控制 Android 设备',
    lede: 'scrcpy 的图形外壳：设备清单、参数面板、连接控制与运行环境，都在一个免安装的桌面程序里。',
    downloadCta: '下载最新版本',
    sourceCta: 'GitHub',
  },
  features: {
    title: '功能特性',
    subtitle: '把 scrcpy 的能力做成可视化的壳。界面上能改的，最终都落到配置里。',
    items: [
      { icon: 'TabletSmartphone', name: '设备清单', desc: '每台设备一个别名，映射到 USB 序列号或 IP:端口，可增删改，在线状态实时可见' },
      { icon: 'Wifi', name: '无线配对与发现', desc: '用配对码完成一次配对，点一下即列出局域网里已开启无线调试的设备' },
      { icon: 'SlidersHorizontal', name: '参数面板', desc: '六组常用选项做成控件，逐项对应 scrcpy 长选项名；每个参数项都能点开「?」看详情，另有自定义参数框' },
      { icon: 'Layers', name: '设备级参数覆盖', desc: '单台设备一套参数，可整体启用或停用' },
      { icon: 'Plug', name: '连接控制', desc: '单台或全部连接 / 断开，多台设备可同时投屏，进程与输出由程序托管' },
      { icon: 'Settings', name: '运行环境', desc: 'scrcpy 与 adb 按需下载，多条通道按序回退并按官方校验值核对；可查看路径、换成已有的目录、检查更新' },
      { icon: 'RefreshCw', name: '应用内更新', desc: '在「关于」里检查并下载新版本，核对后原地替换；便携版自行重启完成，也可设为启动时自动检查' },
      { icon: 'ScrollText', name: '日志', desc: '逐会话输出持久化，保留 7 天 / 30 天 / 永久（卡片在「设置」页）' },
      { icon: 'Terminal', name: '命令预览', desc: '底部显示将执行的完整命令，可一键复制' },
      { icon: 'Palette', name: '外观', desc: '深浅色主题与主题色可切换（默认跟随系统），侧栏可收成图标栏' },
      { icon: 'Languages', name: '界面语言', desc: '设置页可在简体中文与英文之间切换，也可跟随系统' },
    ],
  },
  compare: {
    title: '与同类工具对比',
    subtitle: 'FreeControl2、它的前身 FreeControl，以及同样基于 scrcpy 的 Escrcpy。',
    aspectHead: '对比项',
    columns: ['FreeControl2', 'FreeControl', 'Escrcpy'],
    highlights: [
      { icon: 'HardDrive', stat: '约 6 MB', label: '免安装单文件', desc: 'Windows 程序仅约 6 MB；同为 scrcpy 壳的 Electron 工具约 116 MB。' },
      { icon: 'Cpu', stat: 'Rust 原生核心', label: '轻量高效', desc: '后端由 Rust 原生编译，界面走系统自带的 WebView，不内嵌浏览器内核。' },
      { icon: 'PlugZap', stat: '零配置', label: '开箱即用', desc: 'scrcpy 与 adb 由应用按需获取，不需要手动安装或配置。' },
    ],
    rows: [
      { aspect: '支持平台', fc2: 'Windows 10 / 11 免安装单文件（x64 / ARM64）；Linux 提供 AppImage 与 deb；macOS 提供 dmg（Apple 芯片与 Intel 各一份）', others: ['仅 Windows', 'Windows / macOS / Linux'] },
      { aspect: '程序体积（Windows）', fc2: '约 6 MB（免安装单文件）', others: ['约 7 MB（单文件）', '约 116 MB（安装包）'] },
      { aspect: '维护状态', fc2: '活跃', others: ['已归档，让位于 FreeControl2', '活跃'] },
      { aspect: '设备清单', fc2: '别名映射 USB 序列号或 IP:端口，在线状态实时可见', others: ['USB 与无线 ADB', '多设备协同'] },
      { aspect: '参数控制', fc2: '六组控件逐项对应 scrcpy 长选项，另有自定义参数框；可为单台设备整套覆盖', others: ['常用开关（IME、音频转发、唤醒锁等）', '快捷键与键盘映射'] },
    ],
    note: '对比信息取自各项目的公开资料与发行产物，仅作选型参考；各项目的功能以各自文档为准，体积为对应版本发行产物的即时值。',
  },
  quickstart: {
    title: '快速开始',
    runLabel: '运行要求',
    platforms: [
      { name: 'Windows', req: '10 / 11 · 免安装单文件（x64 / ARM64）· 系统自带 WebView2' },
      { name: 'Linux', req: 'AppImage 或 deb' },
      { name: 'macOS', req: '11 及以上 · dmg（Apple 芯片 / Intel），拖入「应用程序」· Safari 16.2+' },
    ],
    device: '另需一台开启 USB 调试的 Android 设备。',
    macNote: 'macOS 首次打开若被拦下：到「系统设置 → 隐私与安全性」点「仍要打开」，或执行 xattr -dr com.apple.quarantine /Applications/FreeControl2.app。',
    stepsTitle: '四步上手',
    steps: [
      { title: '备好运行环境', desc: '首次启动后到「运行环境」页获取 scrcpy 与 adb。' },
      { title: '添加设备', desc: '在「设备」页新增一台：USB 连接填 adb devices 列出的序列号，无线连接填 IP:端口。' },
      { title: '连接投屏', desc: '在设备卡上点「连接」；多台设备可同时连接。底部命令预览会显示实际下发的完整命令。' },
      { title: '调整参数', desc: '之后可在各参数页调整默认参数，或为某台设备单独开一套覆盖。' },
    ],
    buildTitle: '从源码构建',
    buildNote: '另需 Node.js 24 与 npm、Rust 稳定工具链，以及 Windows 的 MSVC 构建工具、Linux 的 webkit2gtk 等系统库或 macOS 的 Xcode 命令行工具。命令与产物位置见仓库 README。',
  },
  // hero 预览的两处可访问文案：宽屏内嵌页面的名字与窄屏截图的替代文本（随语言成对）
  preview: {
    frameTitle: 'FreeControl2 界面预览',
    shotAlt: 'FreeControl2 设备页',
  },
  faq: {
    title: '常见问题',
    items: [
      {
        q: '设备连不上',
        a: 'USB：确认设备已开启「开发者选项」与「USB 调试」，线缆完好、驱动正常，且 adb devices 能列出它。无线：首次需借助 USB，adb tcpip 5555 后拔线再 adb connect IP:端口；Android 11 及以上也可走「无线调试」的配对码。设备与电脑须在同一局域网。',
      },
      {
        q: '提示找不到 scrcpy 或 adb',
        a: '到「运行环境」页获取。两者都不随安装包分发，按需下载到应用数据目录。',
      },
      {
        q: '下载失败或长时间卡在某个通道',
        a: '直连失败会自动改用后面的通道；也可在进度区就地「切换通道」重下。连不上的通道在本次运行内会被跳过，不再重复等待。',
      },
      {
        q: 'Windows 上启动后界面空白',
        a: '确认系统已装 WebView2 运行时（Windows 10 / 11 一般自带）。',
      },
      {
        q: 'Linux 上启动后界面不再更新',
        a: '虚拟机、软件渲染或远程会话这类环境下，界面可能只画出第一帧、之后就不再更新（点击其实生效）。程序启动时会自行判定并绕开这条渲染路径，这类环境多数无需你干预；若仍然遇到，用 WEBKIT_DISABLE_DMABUF_RENDERER=1 FreeControl2 启动即可 —— 判定是启发式的，有些环境会落空。',
      },
      {
        q: '怎么恢复出厂设置',
        a: '删掉 config.json 再启动即可：设备清单、参数与保留策略都回到出厂默认。',
      },
      {
        q: '退出应用后 adb 还在跑',
        a: '这是有意为之：退出不会结束 adb 服务端，已建立的无线连接随应用退出继续保留，下次启动直接复用。',
      },
    ],
  },
  download: {
    title: '获取 FreeControl2',
    lead: '免费开源，MIT 许可 · Windows 提供 x64 与 ARM64 免安装单文件，Linux 提供 AppImage 与 deb，macOS 提供 dmg。',
    metaLinks: [
      { label: '更新日志', href: 'https://github.com/pdone/FreeControl2/blob/main/Update.md' },
    ],
    cta: '下载',
    // 主按钮那行提示按访客系统与架构取：windows / windowsArm64 是 Windows 的两份产物档，
    // 与平台条目的 os 键不同名（同一个 os 下按架构分两份）；没有真实产物的档位一律落到 Windows x64。
    osHint: {
      windows: 'Windows · x64 · .exe · 免安装单文件',
      windowsArm64: 'Windows · ARM64 · .exe · 免安装单文件',
      linux: 'Linux · x64 · AppImage · 便携版',
      macosArm64: 'macOS · Apple 芯片 · .dmg',
      macosX64: 'macOS · Intel · .dmg',
    },
    yours: '你的系统',
    dlLabel: '下载',
    // 平台卡每行的呈现按该平台的区分维度定：Windows 与 macOS 是同一格式下按架构 / 芯片分两档
    // （`.exe` / `.dmg` 配徽标）；Linux 的区分维度就是格式本身，不给徽标。入口的可见文案统一为
    // 「下载」，可访问名另带所指的那一份 —— 见 App.svelte 里的 aria-label。
    plats: [
      {
        os: 'windows',
        name: 'Windows',
        packages: [
          { key: 'windows', format: '.exe', arch: 'x64', note: '免安装单文件' },
          { key: 'windowsArm64', format: '.exe', arch: 'arm64', note: '免安装单文件' },
        ],
        help: '依赖系统自带的 WebView2。',
      },
      {
        os: 'linux',
        name: 'Linux',
        packages: [
          { key: 'appimage', format: 'AppImage', note: '便携版，通用发行版' },
          { key: 'deb', format: '.deb', note: 'Debian、Ubuntu' },
        ],
      },
      {
        os: 'macos',
        name: 'macOS',
        packages: [
          { key: 'macosArm64', format: '.dmg', arch: 'Apple 芯片', note: '免安装' },
          { key: 'macosX64', format: '.dmg', arch: 'Intel', note: '免安装' },
        ],
        help: 'macOS 11 及以上；首次打开的放行方法见上方「快速开始」。',
      },
    ] as Plat[],
  },
  footer: {
    builtLabel: '构建技术',
    license: { label: 'MIT 开源许可', href: 'https://github.com/pdone/FreeControl2/blob/main/LICENSE' },
    copyright: '© 2026 pdone',
  },
};

export type Dict = typeof zh;

const en: Dict = {
  htmlLang: 'en',
  metaTitle: 'FreeControl2 — Control Android devices on PC',
  nav: {
    features: 'Features',
    compare: 'Comparison',
    quickstart: 'Quick Start',
    faq: 'FAQ',
    download: 'Download',
    switchLabel: '中文',
    switchTitle: '切换到中文',
  },
  hero: {
    tagline: 'Control Android devices on PC',
    lede: 'A graphical shell around scrcpy: device list, parameter panels, connection control and runtime — all in one portable desktop app.',
    downloadCta: 'Download the latest release',
    sourceCta: 'GitHub',
  },
  features: {
    title: 'Features',
    subtitle: 'A visual shell around what scrcpy can do. Everything you change in the UI lands in the config.',
    items: [
      { icon: 'TabletSmartphone', name: 'Device list', desc: 'One alias per device, mapped to a USB serial or IP:port; add, rename and remove, with live online/connected status' },
      { icon: 'Wifi', name: 'Wireless pairing and discovery', desc: 'Pair once with the on-device code, then list the devices with wireless debugging on in one click' },
      { icon: 'SlidersHorizontal', name: 'Parameter panels', desc: 'The common options of six groups as controls, each mapped to its scrcpy long option name; every field has a "?" detail, plus a custom-arguments box' },
      { icon: 'Layers', name: 'Per-device overrides', desc: "A device's own parameter set, switched on or off as a whole" },
      { icon: 'Plug', name: 'Connection control', desc: 'Connect or disconnect one device or all of them; several devices can mirror at once, with processes and output managed by the app' },
      { icon: 'Settings', name: 'Runtime', desc: 'scrcpy and adb fetched on demand, channels tried in order and downloads verified against the official checksums; view the path, switch to an existing directory, check for updates' },
      { icon: 'RefreshCw', name: 'In-app updates', desc: 'Check for and download a new version from "About", verify it and replace in place; portable builds restart to finish, and a startup check can be enabled' },
      { icon: 'ScrollText', name: 'Logs', desc: 'Per-session output persisted, kept for 7 days / 30 days / forever (the cards live on the Settings page)' },
      { icon: 'Terminal', name: 'Command preview', desc: 'The exact command about to run, shown at the bottom and copyable' },
      { icon: 'Palette', name: 'Appearance', desc: 'Light/dark themes and accent color, following the system by default, with a sidebar that collapses to an icon rail' },
      { icon: 'Languages', name: 'Interface language', desc: 'Switch the UI between Chinese and English on the Settings page, or follow the system' },
    ],
  },
  compare: {
    title: 'How it compares',
    subtitle: 'FreeControl2, its predecessor FreeControl, and Escrcpy — another scrcpy-based GUI.',
    aspectHead: 'Aspect',
    columns: ['FreeControl2', 'FreeControl', 'Escrcpy'],
    highlights: [
      { icon: 'HardDrive', stat: '≈ 6 MB', label: 'Single portable file', desc: 'The Windows program is only about 6 MB; an Electron-based scrcpy GUI is about 116 MB.' },
      { icon: 'Cpu', stat: 'Native Rust core', label: 'Light and fast', desc: 'The backend is compiled from Rust and the UI uses the system WebView — no browser engine bundled.' },
      { icon: 'PlugZap', stat: 'Zero setup', label: 'Works out of the box', desc: 'scrcpy and adb are fetched on demand by the app — no manual install or configuration.' },
    ],
    rows: [
      { aspect: 'Platforms', fc2: 'Portable single files for Windows 10 / 11 (x64 / ARM64); AppImage and deb on Linux; dmg for macOS (Apple silicon and Intel)', others: ['Windows only', 'Windows / macOS / Linux'] },
      { aspect: 'Size (Windows)', fc2: '≈ 6 MB (single portable file)', others: ['≈ 7 MB (single file)', '≈ 116 MB (installer)'] },
      { aspect: 'Maintenance', fc2: 'Active', others: ['Archived, superseded by FreeControl2', 'Active'] },
      { aspect: 'Device list', fc2: 'An alias per device mapped to a USB serial or IP:port, with live online status', others: ['USB and wireless ADB', 'Multi-device control'] },
      { aspect: 'Parameters', fc2: 'Six groups of controls mapped to scrcpy long options, a custom-arguments box, and a whole per-device override set', others: ['Common switches (IME, audio forwarding, wake lock…)', 'Shortcuts and keyboard mapping'] },
    ],
    note: 'Taken from each project’s public documentation and release artifacts; for reference only, each project’s own docs are authoritative, and sizes are the current values of the corresponding release artifacts.',
  },
  quickstart: {
    title: 'Quick Start',
    runLabel: 'To run',
    platforms: [
      { name: 'Windows', req: '10 / 11 · single portable file (x64 / ARM64) · system WebView2' },
      { name: 'Linux', req: 'AppImage or deb' },
      { name: 'macOS', req: '11 or later · dmg (Apple silicon / Intel), drag into Applications · Safari 16.2+' },
    ],
    device: 'Plus an Android device with USB debugging enabled.',
    macNote: 'If the first launch on macOS is blocked: open System Settings → Privacy & Security and click Open Anyway, or run xattr -dr com.apple.quarantine /Applications/FreeControl2.app.',
    stepsTitle: 'Four steps',
    steps: [
      { title: 'Get the runtime', desc: 'On first launch, go to the "Runtime" page and fetch scrcpy and adb.' },
      { title: 'Add a device', desc: 'On the "Devices" page, add one: for USB, enter the serial listed by adb devices; for wireless, enter IP:port.' },
      { title: 'Connect and mirror', desc: 'Click "Connect" on the device card. Several devices can be connected at once; the command preview shows the exact command being run.' },
      { title: 'Tune parameters', desc: 'Adjust the defaults on the parameter pages, or give a single device its own override.' },
    ],
    buildTitle: 'Build from source',
    buildNote: 'You additionally need Node.js 24 and npm, a stable Rust toolchain, and the MSVC build tools on Windows, system libraries such as webkit2gtk on Linux, or the Xcode command-line tools on macOS. The commands and artifact locations are in the repository README.',
  },
  preview: {
    frameTitle: 'FreeControl2 interface preview',
    shotAlt: 'FreeControl2 Devices page',
  },
  faq: {
    title: 'FAQ',
    items: [
      {
        q: 'A device will not connect',
        a: 'USB: make sure "Developer options" and "USB debugging" are enabled, the cable is intact, the driver is fine, and adb devices lists it. Wireless: the first time you need USB (adb tcpip 5555, then adb connect IP:port); on Android 11+ you can also use the pairing code under "Wireless debugging". The device and the computer must be on the same network.',
      },
      {
        q: 'It says scrcpy or adb is missing',
        a: 'Fetch them on the "Runtime" page. Neither ships with the app; both are downloaded into the application data directory on demand.',
      },
      {
        q: 'The download fails or hangs on one channel',
        a: 'A failed direct connection falls back to the next channel; you can also switch channels in place. Channels that cannot be reached are skipped for the rest of the run instead of retried.',
      },
      {
        q: 'The window is blank after launch on Windows',
        a: 'Make sure the WebView2 runtime is installed (usually included with Windows 10 / 11).',
      },
      {
        q: 'The window stops updating after launch on Linux',
        a: 'On setups such as virtual machines, software rendering or remote sessions, the window may draw its first frame and then stop updating (clicks still take effect). The app detects this at launch and avoids that rendering path on its own, so most such setups need nothing from you; if you still hit it, start the app with WEBKIT_DISABLE_DMABUF_RENDERER=1 FreeControl2 — the detection is heuristic, so some setups fall through.',
      },
      {
        q: 'How do I reset to the factory defaults',
        a: 'Delete config.json and start the app again: the device list, the parameters and the retention policy all go back to their factory values.',
      },
      {
        q: 'adb is still running after I quit the app',
        a: 'That is intentional: quitting does not stop the adb server, so wireless connections survive the exit and are reused on the next launch.',
      },
    ],
  },
  download: {
    title: 'Get FreeControl2',
    lead: 'Free and open source, MIT licensed · portable single files for Windows x64 and ARM64, AppImage and deb on Linux, dmg for macOS.',
    metaLinks: [
      { label: 'Changelog', href: 'https://github.com/pdone/FreeControl2/blob/main/Update.en.md' },
    ],
    cta: 'Download',
    osHint: {
      windows: 'Windows · x64 · .exe · portable file',
      windowsArm64: 'Windows · ARM64 · .exe · portable file',
      linux: 'Linux · x64 · AppImage · portable',
      macosArm64: 'macOS · Apple silicon · .dmg',
      macosX64: 'macOS · Intel · .dmg',
    },
    yours: 'Your system',
    dlLabel: 'Download',
    plats: [
      {
        os: 'windows',
        name: 'Windows',
        packages: [
          { key: 'windows', format: '.exe', arch: 'x64', note: 'portable file' },
          { key: 'windowsArm64', format: '.exe', arch: 'arm64', note: 'portable file' },
        ],
        help: 'Uses the WebView2 runtime that ships with the system.',
      },
      {
        os: 'linux',
        name: 'Linux',
        packages: [
          { key: 'appimage', format: 'AppImage', note: 'Portable, any distro' },
          { key: 'deb', format: '.deb', note: 'Debian, Ubuntu' },
        ],
      },
      {
        os: 'macos',
        name: 'macOS',
        packages: [
          { key: 'macosArm64', format: '.dmg', arch: 'Apple silicon', note: 'Portable' },
          { key: 'macosX64', format: '.dmg', arch: 'Intel', note: 'Portable' },
        ],
        help: 'macOS 11 or later; see Quick start above for the first-launch note.',
      },
    ] as Plat[],
  },
  footer: {
    builtLabel: 'Built with',
    license: { label: 'MIT License', href: 'https://github.com/pdone/FreeControl2/blob/main/LICENSE' },
    copyright: '© 2026 pdone',
  },
};

export type Lang = 'zh' | 'en';

const STORAGE_KEY = 'freecontrol2-website-lang';

/**
 * 系统首选语言 → 站点语言（spec `project-website`「中英双语成对且可切换」）。
 *
 * 只取首选一项：`fr-FR, zh-CN` 这类系统仍按不受支持的 `fr-FR` 回退中文，不从次选里
 * 挑一个受支持的语言 —— 与产品侧 `src/lib/i18n/resolve.ts` 同一口径，避免界面语言
 * 与访客实际首选不符。非浏览器环境（构建期）同样回退中文。
 */
function systemLang(): Lang {
  if (typeof navigator === 'undefined') return 'zh';
  const primary = navigator.languages?.[0] ?? navigator.language;
  if (!primary) return 'zh';
  const lower = primary.trim().toLowerCase();
  if (lower.startsWith('zh')) return 'zh';
  if (lower.startsWith('en')) return 'en';
  return 'zh';
}

/** 首帧语言：站内选过就用存的，否则按系统首选判定。 */
function initialLang(): Lang {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === 'zh' || saved === 'en') return saved;
  } catch {
    // 存储不可用就按系统语言判定，切换仍在本页生效
  }
  return systemLang();
}

export const lang = writable<Lang>(initialLang());

lang.subscribe((value) => {
  try {
    localStorage.setItem(STORAGE_KEY, value);
  } catch {
    // 存储不可用就不记忆，切换仍在本页生效
  }
});

export const t = derived(lang, ($lang) => ($lang === 'en' ? en : zh));

export function toggleLang(): void {
  lang.update((value) => (value === 'zh' ? 'en' : 'zh'));
}
