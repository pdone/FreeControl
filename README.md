<p align="center">
  <img src="assets/icons/desktop/128x128.png" width="128" height="128" alt="FreeControl2" />
</p>

<h1 align="center">FreeControl2</h1>

<p align="center">
  <a href="https://github.com/pdone/FreeControl2/actions/workflows/build.yml"><img src="https://img.shields.io/github/actions/workflow/status/pdone/FreeControl2/build.yml?style=for-the-badge" alt="构建状态" /></a>
  <a href="https://github.com/pdone/FreeControl2/releases/latest"><img src="https://img.shields.io/github/release/pdone/FreeControl2?style=for-the-badge" alt="最新发行" /></a>
  <a href="https://github.com/pdone/FreeControl2/releases"><img src="https://img.shields.io/github/downloads/pdone/FreeControl2/total?style=for-the-badge" alt="下载量" /></a>
  <a href="https://github.com/pdone/FreeControl2"><img src="https://img.shields.io/github/stars/pdone/FreeControl2?style=for-the-badge" alt="星标" /></a>
  <a href="https://github.com/pdone/FreeControl2/issues"><img src="https://img.shields.io/github/issues/pdone/FreeControl2?style=for-the-badge" alt="议题" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/pdone/FreeControl2?style=for-the-badge" alt="许可证" /></a>
</p>

<p align="center">在PC上控制Android设备</p>

<p align="center">
  中文 | <a href="README.en.md">English</a> | <a href="https://freecontrol2.pages.dev/">官网</a>
</p>

## 功能特性

- **设备清单** —— 每台设备一个别名，映射到 USB 序列号或 `IP:端口`，可增删改，在线状态实时可见
- **无线配对与发现** —— 用配对码完成一次配对，点一下即列出局域网里已开启无线调试的设备
- **参数面板** —— 六组常用选项做成控件，逐项对应 scrcpy 长选项名；每个参数项都能点开「?」看详情，另有自定义参数框
- **设备参数** —— 单台设备一套参数，优先于全局参数，可整体启用或停用
- **连接控制** —— 单台或全部连接 / 断开，多台设备可同时投屏，进程与输出由程序托管
- **运行环境** —— scrcpy 与 adb 按需下载，多条通道按序回退并按官方校验值核对；可查看路径、换成已有的目录、检查更新
- **应用内更新** —— 在「关于」里检查并下载新版本，核对后原地替换；便携版自行重启完成，也可设为启动时自动检查
- **日志** —— 逐会话输出持久化，保留 7 天 / 30 天 / 永久（卡片在「设置」页）
- **命令预览** —— 底部显示将执行的完整命令，可一键复制
- **外观** —— 深浅色主题与主题色可切换（默认跟随系统），侧栏可收成图标栏
- **界面字体与代码字体** —— 设置页可分别选择界面与代码区域的字体，缺字由对应系统默认栈补齐
- **界面语言** —— 设置页可在简体中文与英文之间切换，也可跟随系统

## 界面

**设备页** —— 设备清单与连接控制：

<img src="assets/screenshots/devices.png" alt="设备页：设备清单与连接控制" />

**参数页** —— 常用选项控件：

<img src="assets/screenshots/video.png" alt="视频参数页" />

**运行环境** —— 获取 scrcpy 与 adb 的入口：

<img src="assets/screenshots/environment.png" alt="运行环境页" />

**设置** —— 外观、字体、界面语言、自动更新与日志设置：

<img src="assets/screenshots/logs.png" alt="设置页：外观、字体、界面语言、自动更新与日志设置" />

## 快速开始

**运行要求**：Windows 10 / 11（x64 与 ARM64 各一份免安装单文件）· 系统自带的 WebView2（10 / 11 一般已预装，缺失时用微软的 Evergreen 引导程序安装）· 一台开启了 USB 调试的 Android 设备。Linux 提供 AppImage 与 deb，macOS 11 及以上提供 dmg（Safari 需为 16.2 或更新）。

从 [Releases](https://github.com/pdone/FreeControl2/releases/latest) 获取最新版本：Windows 为免安装单文件，按系统架构选 `FreeControl2_<版本>_windows_x64.exe` 或 `FreeControl2_<版本>_windows_arm64.exe`，放到任意可写目录双击即用 —— 无需安装，也不产出安装器；Linux 提供 AppImage 与 deb 两种发行产物；macOS 提供 dmg（Apple 芯片与 Intel 各一份），打开后把应用拖进「应用程序」。

macOS 首次打开若被 Gatekeeper 拦下，二选一放行：到「系统设置 → 隐私与安全性」点「仍要打开」，或执行 `xattr -dr com.apple.quarantine /Applications/FreeControl2.app`。

1. **备好运行环境** —— 首次启动后到「运行环境」页获取 scrcpy 与 adb。
2. **添加设备** —— 在「设备」页新增一台：USB 连接填 `adb devices` 列出的序列号；无线连接可选「手动 (IP:Port)」直接填地址，或选「局域网扫描」/「配对码」发现并配对设备。添加时可给设备或每条扫描结果设置可选别名。
3. **连接投屏** —— 在设备卡上点「连接」；多台设备可同时连接。底部命令预览会显示实际下发的完整命令。
4. 之后可在各参数页调整全局参数，或为某台设备设置设备参数（优先于全局参数）。

### 从源码构建

另需 Node.js 24 与 npm · Rust 稳定工具链 · Windows 的 MSVC 构建工具 / Linux 的 webkit2gtk 等系统库 / macOS 的 Xcode 命令行工具（清单见 `.github/workflows/build.yml`）。

```sh
npm install
```

Windows（免安装单文件）：

```sh
npm run app:build
```

产物在 `src-tauri/target/release/FreeControl2.exe`。

Linux（AppImage 与 deb）：

```sh
npm run tauri build -- --bundles appimage,deb
```

产物在 `src-tauri/target/release/bundle/`。

macOS（dmg）：

```sh
npm run tauri build -- --bundles dmg
```

产物在 `src-tauri/target/release/bundle/dmg/`。本仓库的 macOS 产物为 ad-hoc 签名、未经公证，首次打开需按上文放行。

手动触发 `.github/workflows/build.yml` 出包时，产物先汇集成**草稿发行**（不自动发布）。草稿的正文取自 `Update.md` 与 `Update.en.md` 中该版本的中英两节，同版本重出时随之刷新。若同一版本已经发布，重出不会覆盖它，而是落到 `v<版本>-draft` 取件草稿 —— 该草稿只用于取回产物，**永不发布**，正文仍是取件说明。

### 本地开发

```sh
npm install
npm run app:dev
```

一条命令起完整应用，前端开发服务器由构建配置自动拉起（`src-tauri/tauri.conf.json` 的 `beforeDevCommand`，界面跑在 `http://localhost:1420`）。常用命令都登记在 `.vscode/tasks.json` 的编辑器任务里，从任务列表按名字直接跑即可。

## 项目结构

```
FreeControl2/
├─ src/                    前端（Svelte + Vite + TypeScript）
│  ├─ components/          界面组件（设备页、各参数页、设置与日志、运行环境、关于弹窗…）
│  └─ lib/                 前端逻辑（参数模型、命令拼装、校验、配置存取）
├─ src-tauri/              后端（Rust + Tauri 2）
│  ├─ src/                 命令层与平台实现（设备、scrcpy、配置、资产、下载、会话、日志）
│  └─ tauri.conf.json      应用与打包配置 —— 版本号的单一来源
├─ assets/icons/           品牌资产包（唯一来源，含图标生成流程与用法说明）
├─ scripts/                图标同步脚本（按打包清单镜像到 src-tauri/icons/）
├─ .vscode/                编辑器任务入口（只引用 package.json 的脚本名）
├─ ui-design/              UI 基线原型（只读参考）
├─ openspec/               规格与变更工件
└─ Update.md               更新日志
```

## 常见问题

### 设备连不上

**USB** —— 确认设备已开启「开发者选项」与「USB 调试」，数据线完好，Windows 下驱动正常（设备管理器里该设备没有黄色感叹号），且 `adb devices` 能列出它。找不到开发者选项时，在「设置」-「关于手机」里连点「版本号」7 次即可显示。

**无线** —— 在「添加设备」里选择「局域网扫描」或「配对码」，也可选「手动 (IP:Port)」直接填写连接地址。Android 11 及以上先在设备的「无线调试」里点「使用配对码配对设备」，应用会查找待配对设备并填入配对地址（多台时需选择），填 6 位配对码即可；找不到设备时也可手填配对地址。应用会自行解析连接端口并建链。扫描结果和配对表单都可以为设备设置可选别名，添加后无需再去设备卡改名。连接端口会随设备重启或重开无线调试而变化，设备变离线时点该设备卡上的「重新解析端口」。仍在使用 `adb tcpip 5555` 的设备可以直接扫到，也可以手填 `IP:端口`。设备与电脑须在同一局域网。

### 提示找不到 scrcpy 或 adb

到「运行环境」页获取。两者都不随安装包分发，默认按需下载到应用数据目录；也可以在该页把目录改为已有的自定义目录。

### 怎么更新到新版本

打开「关于」点「检查更新」，或在「设置」页打开「自动检查更新」—— 程序会下载并核对新版本，替换后自行重启。Linux 的安装包版本下载完成后交给系统安装程序。macOS 的 dmg 下载并校验后由 Finder 打开，把应用拖进「应用程序」覆盖旧版即可，应用不会自行退出。

### 下载失败或长时间卡在某个通道

直连失败会自动改用后面的通道；也可在进度区就地「切换通道」重下。连不上的通道在本次运行内会被跳过，不再重复等待。

### Windows 上启动后界面空白

确认系统已装 WebView2 运行时（Windows 10 / 11 一般自带）。

### Linux 上启动后界面不再更新

虚拟机、软件渲染或远程会话这类环境下，界面可能只画出第一帧、之后就不再更新（点击其实生效）。程序启动时会自行判定并绕开这条渲染路径，这类环境多数无需你干预；若仍然遇到，用 `WEBKIT_DISABLE_DMABUF_RENDERER=1 FreeControl2` 启动即可 —— 判定是启发式的，有些环境会落空。

### 数据都存在哪

都在应用数据根下（Windows 为 `%APPDATA%/FreeControl2`，Linux 为 XDG 数据根下的同名目录，macOS 为 `~/Library/Application Support/FreeControl2`）：

| 路径 | 用途 |
|---|---|
| `config.json` | 单一配置文件（含 `schemaVersion`）：设备清单、全局参数、设备参数、日志保留策略 |
| `logs/yyyy-MM-dd.log` | 会话输出与本程序关键操作的持久化记录 |
| `scrcpy-{OS}-v{版本}/` | 按需下载的 scrcpy 与 adb 运行环境（未设置自定义目录时） |

界面上能改的东西都落在 `config.json`。

### 怎么恢复出厂设置

删掉 `config.json` 再启动即可：设备清单、参数与保留策略都回到出厂默认。

### 日志在哪

「设置」页可一键打开日志目录；文件位于应用数据根下的 `logs/`，按日期命名。

### 退出应用后 adb 还在跑

这是有意为之：退出不会结束 adb 服务端，已建立的无线连接随应用退出继续保留，下次启动直接复用。

## 许可证

[MIT](LICENSE)

## 致谢

- [scrcpy](https://github.com/Genymobile/scrcpy) —— 投屏与设备控制全部由它完成，本项目是它的图形外壳。
- [Tauri](https://tauri.app/)、[Svelte](https://svelte.dev/)、[Vite](https://vite.dev/)、[lucide](https://lucide.dev/) —— 应用外壳与界面。
- 前身 [FreeControl](https://github.com/pdone/FreeControl) —— 本项目在它的交互经验上重做。
- [rayburst](https://github.com/AnInsomniacy/rayburst) —— 本项目部分功能设计参考了此项目。
- [escrcpy](https://github.com/viarotel-org/escrcpy) —— 本项目部分功能设计参考了此项目。
