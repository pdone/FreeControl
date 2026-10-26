# Tauri 2 图标与 Logo 资产包

唯一真源：`logo/logo-master.png`（1024×1024）。方底是自左上 `#4a4a48` 到右下 `#262626` 的亮度渐变，图形主色 `#FCCD01`（带明暗层次），圆角半径 243/1024 ≈ 23.7%，方底之外是真透明。

## 1. 生成流程

```
logo/logo-master.png --(python assets/icons/build_icons.py)--> desktop/ logo/ tray/ android/ ios/ preview.png
```

在**仓库根目录**执行：

```bash
python assets/icons/build_icons.py
```

依赖：`numpy`、`Pillow`、`scipy`。

输入只有母版，输出直接落到本包各分区 —— **换设计时只替换母版、再跑这一条命令**，不需要逐个文件手改，也不经过仓库之外的暂存目录。

### 母版本身的由来（换设计时的一次性步骤）

母版不是能直接用的原始出图，需要三步处理：

1. **按饱和度分离**底板与图形（底板近中性、图形高饱和，两者之间有很宽的安全区）；
2. **底板**做亮度线性重映射到 `[38, 74]`（`#262626`..`#4a4a48`）并把两端像素钉成端点色；**图形**保留明暗层次、色相归正到主色；
3. 套 243/1024 圆角遮罩生成真 alpha，抗锯齿过渡带做 defringe。

这三步的取舍与实测数据记在本次换代的 `openspec/changes/brand-icon-refresh/design.md`（D1 / D1b）。

> 为什么到处都要 defringe：AI 出图的透明区 RGB 常被压成近黑，缩放插值时会把黑色渗进边缘；defringe 在缩放前后各填一次「最近实心像素的颜色」，透明区就没有黑色可渗。同理，`resize()` 里还有一步**地板回抬** —— LANCZOS 核带负瓣，方底与图形的高对比边缘会下冲，把方底压到地板之下（实测旧成品 32px 最低 31.4），按亮度把方底像素抬回地板。

## 2. 目录与落位

| 本包目录 | 拷到项目中的位置 | 说明 |
|---|---|---|
| `desktop/` | `src-tauri/icons/` | 桌面端（Windows / macOS / Linux）打包图标 |
| `android/res/` | `src-tauri/gen/android/app/src/main/res/` | Android，需先 `cargo tauri android init` |
| `ios/` | `src-tauri/gen/apple/Assets.xcassets/AppIcon.appiconset/` | iOS，需先 `cargo tauri ios init` |
| `tray/` | `src-tauri/icons/`（或前端 `public/`） | 托盘图标变体，v1 未使用 |
| `logo/` | 不进构建产物 | 设计源与界面用件 |

桌面图标的落位由 `scripts/sync-icons.mjs` 按 `src-tauri/tauri.conf.json` 的 `bundle.icon` 清单**镜像同步**完成（`npm run icons`），不需要手工拷贝。同步是「先清空再拷贝」，换设计后旧图标不会残留。清单是唯一事实来源，脚本不硬编码文件名。

`logo/` 只留母版与透明底图形（设计源），不进构建产物；界面标识与 favicon 改为直接引用 `desktop/` 的成品位图，不另存副本。

## 3. 桌面端成品

- **`icon.ico`**：7 层（16 / 24 / 32 / 48 / 64 / 128 / 256），目录项按尺寸升序写入。小于 256 的层用**未压缩位图**（BMP），256 层用 PNG 压缩 —— 小尺寸层也压缩时，并非所有「按尺寸取层」的调用方都能取到该尺寸的位图。窗口图标**不依赖首层是哪一个尺寸**：运行期按显示器当前 DPI 从这份 ICO 取原生层供给（16×缩放给标题栏、32×缩放给任务栏）。
- **`icon.icns`**：11 层（icp4 / icp5 / icp6 / ic07 ~ ic14，16px ~ 1024px）。
- **`icon.png`**：1024×1024，Linux 与各平台兜底。
- **`Square*Logo.png` / `StoreLogo.png`**：AppX / Microsoft Store 目标专用，日常构建用不到但建议保留。
- 各尺寸 PNG 都是**原生重采样**结果，不存在由其它尺寸放大凑出的层。

### 哪些文件真的会进 exe

**只进 `bundle.icon` 数组列出的那几个**，其余目录（`android/`、`ios/`、`logo/`、`tray/`，以及 `desktop/` 下没被列进数组的 PNG）不会自动打包，除非显式写进 `bundle.resources`。

各目标按格式挑用：Windows 取 `.ico`（嵌入 exe 的 RT_ICON 资源），macOS 取 `.icns`，Linux 取 `.png`。但数组里列出的文件会被一并复制进产物资源区，所以只做 Windows 包时可以精简，省掉最大的 `icon.icns`：

```json
{
  "bundle": {
    "icon": ["icons/32x32.png", "icons/icon.ico"]
  }
}
```

要跨平台发布就别精简 —— Tauri 官方也建议始终保留全套。

需要把 `logo/`、`tray/` 等一并打进产物时用 `resources`（可用 glob），运行时用 `$RESOURCE` 解析，不要拼相对路径：

```rust
let path = app.path().resolve("desktop/128x128.png", BaseDirectory::Resource)?;
```

Linux（`deb` / `appimage`）会把列出的 PNG 装到 `/usr/share/icons/hicolor/<size>/apps/`；移动端 `android/`、`ios/` 只进 apk / ipa，与桌面 exe 无关。

## 4. 前端窗口 / 托盘图标

托盘图标建议用透明背景版本，避免深色托盘上出现色块；macOS 必须用单色模板图，否则会被强制渲染成黑色剪影。

⚠️ 不要用 `Image::from_path("icons/...")`：它按**当前工作目录**解析，打包后 cwd 通常是安装目录或 `C:\Windows\System32`，图标会加载失败。用编译期嵌入：

```rust
use tauri::{image::Image, tray::TrayIconBuilder, Manager};

// 编译期把 PNG 解码成原始像素嵌进二进制，路径相对 CARGO_MANIFEST_DIR（即 src-tauri/）
// 按原图尺寸嵌入即可，不要用大图 —— 体积按原图尺寸线性膨胀
#[cfg(target_os = "macos")]
const TRAY_ICON: Image<'_> = include_image!("icons/tray-mac-template@2x.png");
#[cfg(not(target_os = "macos"))]
const TRAY_ICON: Image<'_> = include_image!("icons/tray-64-transparent.png");

fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id("main")
        .icon(TRAY_ICON)
        .icon_as_template(true)   // macOS 关键：作为模板图渲染
        .tooltip("FreeControl")
        .build(app)?;
    Ok(())
}
```

`Cargo.toml` 需启用 tray 特性：

```toml
tauri = { version = "2", features = ["tray-icon", "image-png"] }
```

若托盘图标要跟随主题切换，前端监听 `getCurrentWindow().theme()`，Rust 侧 `tray.set_icon(...)` 切换 `tray-64-white.png` / `tray-64-black.png` —— 同样用 `include_image!` 预定义两个常量。

> `tray/` 的这几个变体由生成流程按母版产出（`tray-64-color` 是底板变体，其余是图形剪影的单色染色）。v1 未启用托盘图标，这批文件只在包内保留；将来启用时注意地板值在深色托盘上的可辨识度。

## 5. Logo 用法建议

- `logo-master.png`：1024×1024 完整方形图标，一切图标再生成的**唯一真源**；
- `logo-transparent.png`：透明底的位图版本（由生成流程按饱和度抠出图形）。

界面标识与应用窗口图标是**同一品牌符号**（同一母版、同一轮廓、同一底板、同一圆角）。界面侧取 `desktop/` 的**成品位图**（侧边栏 128、关于弹窗 256）—— 位图保留母版的明暗层次，且都是原生重采样，不是放大低位图凑的；界面里不复现品牌色值，尺寸与品牌行偏移统一由 CSS 变量 `--logo` 派生。

## 6. 移动端

Android 已按 adaptive icon 规范生成：`ic_launcher_foreground.png` 为 108dp 画布、图形居中在 66dp 安全区内，四周留空，系统遮罩（圆 / 水滴 / 方）不会裁掉主体；`ic_launcher_background.xml` 是**纯色**层，取方底亮度带的中点 `#383838`（这一层只能填纯色，渐变由前景层与遮罩共同呈现）。

iOS 成品**满幅不透明**（App Store 与 Xcode 校验要求），四角不套圆角遮罩，由平台自加。注意不要用「垫一层纯色再合成」的老办法：那要求垫底色恰好等于方底色，渐变方底的四个角亮度各不相同，垫任何单色都会在角上露出异色缺口。

两个平台都要在 `cargo tauri android/ios init` 之后再落位。

## 7. 文件清单

```
desktop/    19 个（PNG × 17 + icon.ico + icon.icns）
android/    18 个（mipmap-mdpi ~ xxxhdpi 各 3 张 + values/背景色 + 2 个 anydpi-v26 配置）
ios/        16 个（PNG × 15 + Contents.json）
tray/        5 个托盘变体
logo/        2 个（母版 PNG、透明版 PNG）
preview.png  对照图：上排各尺寸桌面图标，下排托盘变体（棋盘格底衬）
```

`preview.png` 由生成流程一并产出，只作人工核对用，不进任何构建产物。
