# FreeControl

[![](https://img.shields.io/github/actions/workflow/status/pdone/FreeControl/build-and-release.yml?style=for-the-badge)](https://github.com/pdone/FreeControl/actions/workflows/build-and-release.yml)
[![](https://img.shields.io/github/release/pdone/FreeControl?style=for-the-badge)](https://github.com/pdone/FreeControl/releases/latest)
[![](https://img.shields.io/github/downloads/pdone/FreeControl/total?style=for-the-badge)](https://github.com/pdone/FreeControl/releases)
[![](https://img.shields.io/github/stars/pdone/FreeControl?style=for-the-badge)](https://github.com/pdone/FreeControl)
[![](https://img.shields.io/github/issues/pdone/FreeControl?style=for-the-badge)](https://github.com/pdone/FreeControl/issues)

## 介绍

[ 中文 | [English](https://github.com/pdone/FreeControl/blob/master/README.en.md) ]

基于开源项目[**scrcpy**](https://github.com/Genymobile/scrcpy)，提供简洁的交互界面。

- 编码语言 `C#`
- 开发工具 `Visual Studio 2022`
- 运行环境 `.NET Framework 4.7.2`

## 界面

![](https://raw.githubusercontent.com/pdone/static/master/img/article/free-control/1.7.0_1.png)

![](https://raw.githubusercontent.com/pdone/static/master/img/article/free-control/1.7.0_2.png)

![](https://raw.githubusercontent.com/pdone/static/master/img/article/free-control/v1.4.0_5.gif)

## 下载
### GitHub Release

https://github.com/pdone/FreeControl/releases/latest/download/FreeControl.exe

### My Proxy

https://cdn.awaw.cc/gh/pdone/FreeControl/releases/latest/download/FreeControl.exe

## 代码存储库

[![](https://img.shields.io/badge/github-Free_Control-blue?style=for-the-badge&logo=github)](https://github.com/pdone/FreeControl)

## 更新记录

[![](https://img.shields.io/badge/updete-record-fedcba?style=for-the-badge)](https://github.com/pdone/FreeControl/blob/master/FreeControl/Update.md)

## 常见问题

### 关闭窗口后锁屏

`v1.7.1` 及以后的版本中，增加 `PowerOffOnClose` 参数，用于控制关闭控制窗口后，是否将手机锁屏。默认不启用，可以在配置文件 `%AppData%\FreeControl\config.json` 中，将 `PowerOffOnClose` 参数值改为 `true` 以启用。

### 编译问题

本仓库已添加 `Workflows` 进行持续集成，可通过 `GitHub Action` 查看最新代码构建情况。如果 `Workflows` 构建成功，但拉取本地后无法正常编译，可尝试手动添加项目依赖，也可参考 `Workflows` 中配置的构建流程重试。

### 输入法问题

`v1.7.0` 及以后的版本中，默认启用scrcpy的 `UHID keyboard` 特性，以优化中文输入体验。

如果在此过程中遇到了什么问题，可尝试打开配置文件 `%AppData%\FreeControl\config.json`，将 `CustomArgs` 的值由 `--keyboard=uhid` 改为 `--keyboard=sdk`。

更多信息可参考 [scrcpy-doc-keyboard](https://github.com/Genymobile/scrcpy/blob/master/doc/keyboard.md)。

<details>
<summary>已过时</summary>

个人测试发现，目前支持跨屏进行拼音输入的 **手机输入法APP** 如下：

- 搜狗输入法
- QQ输入法
- 谷歌拼音输入法
- Gboard
- 微信输入法

此功能需要输入法APP适配，有更好用的输入法欢迎留言推荐。

> 自动切换输入法功能默认禁用。如需启用，先关闭程序，然后打开配置文件 `%AppData%\FreeControl\config.json`，将 `EnableSwitchIME` 字段值改为 `true`，保存后启动程序即可。

</details>

### 设备连接问题

#### USB 连接

通过 USB 连接来正常使用 adb 需要保证几点：

1. 硬件状态正常。

   包括 Android 设备处于正常开机状态，USB 连接线和各种接口完好。

2. Android 设备的开发者选项和 USB 调试模式已开启。

   可以到「设置」-「开发者选项」-「Android 调试」查看。

   如果在设置里找不到开发者选项，那需要通过一个彩蛋来让它显示出来：在「设置」-「关于手机」连续点击「版本号」7 次。

3. 设备驱动状态正常。

   这一点貌似在 Linux 和 Mac OS X 下不用操心，在 Windows 下有可能遇到需要安装驱动的情况，确认这一点可以右键「计算机」-「属性」，到「设备管理器」里查看相关设备上是否有黄色感叹号或问号，如果没有就说明驱动状态已经好了。否则可以下载一个手机助手类程序来安装驱动先。

4. 通过 USB 线连接好电脑和设备后确认状态。

   ```sh
   adb devices
   ```

   如果能看到

   ```sh
   6d56e83a device
   ```

   说明连接成功。

#### 无线连接（Android11 及以上）

Android 11 及更高版本支持使用 Android 调试桥 (adb) 从工作站以无线方式部署和调试应用。例如，您可以将可调试应用部署到多台远程设备，而无需通过 USB 实际连接设备。这样就可以避免常见的 USB 连接问题，例如驱动程序安装方面的问题。

[官方文档](https://developer.android.com/studio/command-line/adb?hl=zh_cn#connect-to-a-device-over-wi-fi-android-11+)

操作步骤：

1. 更新到最新版本的 [SDK 平台工具](https://developer.android.com/studio/releases/platform-tools?hl=zh_cn)(至少30.0.0)。

2. 将 Android 设备与要运行 adb 的电脑连接到同一个局域网，比如连到同一个 WiFi。

3. 在开发者选项中启用**无线调试**。

4. 在询问要允许在此网络上进行无线调试吗？的对话框中，点击允许。

5. 选择使用配对码配对设备，使用弹窗中的 IP 地址和端口号。

    ```sh
    adb pair ipaddr:port
    ```

6. 提示 `Enter pairing code:` 时输入弹窗中的配对码，成功后会显示  `Successfully paired to ...` 。

7. 使用无线调试下的 **IP 地址和端口**。

    ```sh
    adb connect ipaddr:port
    ```

8. 确认连接状态。

   ```sh
   adb devices
   ```

   如果能看到

   ```sh
   ipaddr:port device
   ```

   说明连接成功。

#### 无线连接（需要借助 USB 线）

除了可以通过 USB 连接设备与电脑来使用 adb，也可以通过无线连接——虽然连接过程中也有需要使用 USB 的步骤，但是连接成功之后你的设备就可以在一定范围内摆脱 USB 连接线的限制啦！

操作步骤：

1. 将 Android 设备与要运行 adb 的电脑连接到同一个局域网，比如连到同一个 WiFi。

2. 将设备与电脑通过 USB 线连接。

   应确保连接成功（可运行 `adb devices` 看是否能列出该设备）。

3. 让设备在 5555 端口监听 TCP/IP 连接：

   ```sh
   adb tcpip 5555
   ```

4. 断开 USB 连接。

5. 找到设备的 IP 地址。

   一般能在「设置」-「关于手机」-「状态信息」-「IP地址」找到。

6. 通过 IP 地址连接设备。

   ```sh
   adb connect <device-ip-address>
   ```

   这里的 `<device-ip-address>` 就是上一步中找到的设备 IP 地址。

7. 确认连接状态。

   ```sh
   adb devices
   ```

   如果能看到

   ```sh
   <device-ip-address>:5555 device
   ```

   说明连接成功。

如果连接不了，请确认 Android 设备与电脑是连接到了同一个 WiFi，然后再次执行 `adb connect <device-ip-address>` 那一步；

如果还是不行的话，通过 `adb kill-server` 重新启动 adb 然后从头再来一次试试。

#### 无线连接（无需借助 USB 线）

**需要 root 权限**，此处不做详细说明，有需要的朋友可[参考此处](https://github.com/mzlogin/awesome-adb#%E6%97%A0%E7%BA%BF%E8%BF%9E%E6%8E%A5%E6%97%A0%E9%9C%80%E5%80%9F%E5%8A%A9-usb-%E7%BA%BF)。

### 保持唤醒功能

仅在手机充电时有效。

### 音频转发功能

使用 `Android 11` 或更高版本的设备支持音频转发，并且默认情况下启用:

- 对于 `Android 12` 或更高版本，它开箱即用。
- 对于 `Android 11` ，您需要确保在启动scrcpy时设备屏幕已解锁。假的弹出窗口将短暂出现，使系统认为shell应用程序处于前台。没有这个，音频捕获将失败。
- 对于 `Android 10` 或更早版本，无法捕获音频并自动禁用。

## 捐赠
如果您觉得这个项目对您有帮助，欢迎请作者喝杯咖啡。☕

<details>
<summary>展开</summary>

![](https://raw.githubusercontent.com/pdone/static/master/img/donate/zfb_wx.jpg)

爱发电❤ https://afdian.net/a/pdone
</details>

## Stargazers Over Time
[![Stargazers over time](https://starchart.cc/pdone/FreeControl.svg?variant=adaptive)](https://starchart.cc/pdone/FreeControl)
