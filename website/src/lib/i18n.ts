import { derived, writable } from 'svelte/store';

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
      { icon: 'SlidersHorizontal', name: '参数面板', desc: '六组常用选项做成控件，逐项对应 scrcpy 长选项名，另有自定义参数框' },
      { icon: 'Layers', name: '设备级参数覆盖', desc: '单台设备一套参数，可整体启用或停用' },
      { icon: 'Plug', name: '连接控制', desc: '单台或全部连接 / 断开，多台设备可同时投屏，进程与输出由程序托管' },
      { icon: 'Settings', name: '运行环境', desc: 'scrcpy 与 adb 按需下载，可查看路径、打开目录、检查更新' },
      { icon: 'ArrowDownToLine', name: '下载通道', desc: '直连、内置代理与自加代理按序回退，当前所用通道在界面上标明' },
      { icon: 'ScrollText', name: '日志', desc: '逐会话输出持久化，保留 7 天 / 30 天 / 永久' },
      { icon: 'Terminal', name: '命令预览', desc: '底部显示将执行的完整命令，可一键复制' },
      { icon: 'Palette', name: '外观', desc: '暗色优先，侧栏可收成图标栏' },
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
      { aspect: '支持平台', fc2: 'Windows 10 / 11 免安装单文件；Linux 提供 AppImage 与 deb', others: ['仅 Windows', 'Windows / macOS / Linux'] },
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
    requirements: 'Windows 10 / 11（x64；Windows on ARM 上经 x64 仿真运行）· 系统自带的 WebView2 · 一台开启了 USB 调试的 Android 设备。Linux 上另提供 AppImage 与 deb 发行产物。',
    portable: '程序是免安装的单文件：从发行页下载 FreeControl2_<版本>.exe，放到任意可写目录双击即用 —— 无需安装，也不产出安装器。',
    stepsTitle: '四步上手',
    steps: [
      { title: '备好运行环境', desc: '首次启动后到「运行环境」页获取 scrcpy 与 adb。' },
      { title: '添加设备', desc: '在「设备」页新增一台：USB 连接填 adb devices 列出的序列号，无线连接填 IP:端口。' },
      { title: '连接投屏', desc: '在设备卡上点「连接」；多台设备可同时连接。底部命令预览会显示实际下发的完整命令。' },
      { title: '调整参数', desc: '之后可在各参数页调整默认参数，或为某台设备单独开一套覆盖。' },
    ],
    buildTitle: '从源码构建',
    buildNote: '另需 Node.js 24 与 npm、Rust 稳定工具链，以及 Windows 的 MSVC 构建工具或 Linux 的 webkit2gtk 等系统库。命令与产物位置见仓库 README。',
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
    lead: '免费开源，MIT 许可 · Windows 免安装单文件，Linux 提供 AppImage 与 deb。',
    metaLinks: [
      { label: '更新日志', href: 'https://github.com/pdone/FreeControl2/blob/main/Update.md' },
    ],
    cta: '下载',
    osHint: 'Windows · x64 · .exe · 免安装单文件',
    yours: '你的系统',
    dlLabel: '下载',
    plats: [
      {
        os: 'windows',
        name: 'Windows',
        packages: [{ key: 'windows', format: '.exe', note: '免安装单文件' }],
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
    ],
    note: 'Windows 免安装、不产出安装器；Linux 提供 AppImage 与 deb 发行产物。',
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
      { icon: 'TabletSmartphone', name: 'Device list', desc: 'One alias per device, mapped to a USB serial or IP:port; add, rename and remove, with live online status' },
      { icon: 'SlidersHorizontal', name: 'Parameter panels', desc: 'The common options of six groups as controls, each mapped to its scrcpy long option name, plus a custom-arguments box' },
      { icon: 'Layers', name: 'Per-device overrides', desc: "A device's own parameter set, switched on or off as a whole" },
      { icon: 'Plug', name: 'Connection control', desc: 'Connect or disconnect one device or all of them; several devices can mirror at once' },
      { icon: 'Settings', name: 'Runtime', desc: 'scrcpy and adb fetched on demand; view the path, open the directory, check for updates' },
      { icon: 'ArrowDownToLine', name: 'Download channels', desc: 'Direct, built-in proxy and your own proxies, tried in order, with the channel in use shown in the UI' },
      { icon: 'ScrollText', name: 'Logs', desc: 'Per-session output persisted, kept for 7 days / 30 days / forever' },
      { icon: 'Terminal', name: 'Command preview', desc: 'The exact command about to run, shown at the bottom and copyable' },
      { icon: 'Palette', name: 'Appearance', desc: 'Dark-first, with a sidebar that collapses to an icon rail' },
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
      { aspect: 'Platforms', fc2: 'Windows 10 / 11 as a single portable file; AppImage and deb on Linux', others: ['Windows only', 'Windows / macOS / Linux'] },
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
    requirements: 'Windows 10 / 11 (x64; on Windows on ARM it runs under x64 emulation) · the WebView2 runtime that ships with the system · an Android device with USB debugging enabled. On Linux the project also provides AppImage and deb release artifacts.',
    portable: 'The app is a single portable executable: download FreeControl2_<version>.exe from Releases and run it from any writable directory — there is no installer.',
    stepsTitle: 'Four steps',
    steps: [
      { title: 'Get the runtime', desc: 'On first launch, go to the "Runtime" page and fetch scrcpy and adb.' },
      { title: 'Add a device', desc: 'On the "Devices" page, add one: for USB, enter the serial listed by adb devices; for wireless, enter IP:port.' },
      { title: 'Connect and mirror', desc: 'Click "Connect" on the device card. Several devices can be connected at once; the command preview shows the exact command being run.' },
      { title: 'Tune parameters', desc: 'Adjust the defaults on the parameter pages, or give a single device its own override.' },
    ],
    buildTitle: 'Build from source',
    buildNote: 'You additionally need Node.js 24 and npm, a stable Rust toolchain, and the MSVC build tools on Windows or system libraries such as webkit2gtk on Linux. The commands and artifact locations are in the repository README.',
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
    lead: 'Free and open source, MIT licensed · one portable file on Windows, AppImage and deb on Linux.',
    metaLinks: [
      { label: 'Changelog', href: 'https://github.com/pdone/FreeControl2/blob/main/Update.md' },
    ],
    cta: 'Download',
    osHint: 'Windows · x64 · .exe · portable file',
    yours: 'Your system',
    dlLabel: 'Download',
    plats: [
      {
        os: 'windows',
        name: 'Windows',
        packages: [{ key: 'windows', format: '.exe', note: 'Single portable file' }],
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
    ],
    note: 'Portable on Windows with no installer; AppImage and deb release artifacts on Linux.',
  },
  footer: {
    builtLabel: 'Built with',
    license: { label: 'MIT License', href: 'https://github.com/pdone/FreeControl2/blob/main/LICENSE' },
    copyright: '© 2026 pdone',
  },
};

export type Lang = 'zh' | 'en';

const STORAGE_KEY = 'freecontrol2-website-lang';

function initialLang(): Lang {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === 'zh' || saved === 'en') return saved;
  } catch {
    // 存储不可用时退回默认语言
  }
  return 'zh';
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
