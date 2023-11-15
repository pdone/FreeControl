# FreeControl

## 介绍
基于开源项目[**scrcpy**](https://github.com/Genymobile/scrcpy)，提供简洁的交互界面。

- 编码语言 `C#`
- 开发工具 `Visual Studio 2022`
- 运行环境 `.NET Framework 4.7.2`

## 截图

![](https://cdn.jsdelivr.net/gh/pdone/static@latest/img/article/free-control/1.6.5_1.png)

![](https://cdn.jsdelivr.net/gh/pdone/static@latest/img/article/free-control/1.6.5_2.png)

![](https://cdn.jsdelivr.net/gh/pdone/static@latest/img/article/free-control/v1.4.0_5.gif)

## 下载
### GitHub Release

[https://github.com/pdone/FreeControl/releases/latest/download/FreeControl.exe](https://github.com/pdone/FreeControl/releases/latest/download/FreeControl.exe)

### ghproxy代理加速

[https://ghproxy.com/https://github.com/pdone/FreeControl/releases/latest/download/FreeControl.exe](https://ghproxy.com/https://github.com/pdone/FreeControl/releases/latest/download/FreeControl.exe)

## 代码

[https://github.com/pdone/FreeControl](https://github.com/pdone/FreeControl)

## FAQ

### Q1 输入法问题

个人测试发现，目前支持跨屏进行拼音输入的 **手机输入法APP** 如下：

- 搜狗输入法
- QQ输入法
- 谷歌拼音输入法
- Gboard

此功能需要输入法APP适配，暂时只发现这4款，有更好用的输入法欢迎留言推荐。

#### ⭐自动切换输入法功能说明

由于大多数朋友日常使用的输入法不支持跨屏输入，所以增加了启动时自动切换输入法的功能，用于自动切换到非日常使用、但支持跨屏输入的输入法。

在主界面中设置启动时要切换的输入法，点击启动按钮便会自动切换（确保手机上已安装对应输入法APP）。

关闭窗口时，会切换回原来的输入法。

### Q2 无线访问问题

#### 无线连接（Android11 及以上）

Android 11 及更高版本支持使用 Android 调试桥 (adb) 从工作站以无线方式部署和调试应用。例如，您可以将可调试应用部署到多台远程设备，而无需通过 USB 实际连接设备。这样就可以避免常见的 USB 连接问题，例如驱动程序安装方面的问题。

***操作步骤：***

1. 更新到最新版本的 SDK 平台工具(至少30.0.0)。
2. 将 Android 设备与要运行 adb 的电脑连接到同一个局域网，比如连到同一个 WiFi。
3. 在开发者选项中启用无线调试。
4. 在询问要 `允许在此网络上进行无线调试吗？` 的对话框中，点击允许。
5. 选择使用配对码配对设备，使用弹窗中的 IP 地址和端口号。
	```
	adb pair ipaddr:port
	```
6. 提示 `Enter pairing code:` 时输入弹窗中的配对码，成功后会显示 `Successfully paired to ...` 。
7. 使用无线调试下的 **IP 地址和端口**。
	```
	adb connect ipaddr:port
	```
8. 确认连接状态。
	```
	adb devices
	```
	如果能看到 ` ipaddr:port device` 说明连接成功。

#### 无线连接（需要借助 USB 线）

除了可以通过 USB 连接设备与电脑来使用 adb，也可以通过无线连接——虽然连接过程中也有需要使用 USB 的步骤，但是连接成功之后你的设备就可以在一定范围内摆脱 USB 连接线的限制啦！

***操作步骤：***

1. 将 Android 设备与要运行 adb 的电脑连接到同一个局域网，比如连到同一个 WiFi。
2. 将设备与电脑通过 USB 线连接。
   应确保连接成功（可运行 `adb devices` 看是否能列出该设备）。
3. 让设备在 5555 端口监听 TCP/IP 连接：
	```
	adb tcpip 5555
	```
4. 断开 USB 连接。
5. 找到设备的 IP 地址。
   一般能在「设置」-「关于手机」-「状态信息」-「IP地址」找到，也可以使用下文里 [查看设备信息 - IP 地址](https://github.com/mzlogin/awesome-adb#ip-地址) 一节里的方法用 adb 命令来查看。
6. 通过 IP 地址连接设备。
	```
	adb connect <device-ip-address>
	```
	这里的 `<device-ip-address>` 就是上一步中找到的设备 IP 地址。
7. 确认连接状态。
   ```
   adb devices
   ```
   如果能看到 `<device-ip-address>:5555 device` 说明连接成功。

   如果连接不了，请确认 Android 设备与电脑是连接到了同一个 WiFi，然后再次执行 `adb connect <device-ip-address>` 那一步；

   如果还是不行的话，通过 `adb kill-server` 重新启动 adb 然后从头再来一次试试。

#### 无线连接（无需借助 USB 线）
此方式**需要设备root**，此处不做详细说明，有需要的朋友可[参考此处](https://github.com/mzlogin/awesome-adb#%E6%97%A0%E7%BA%BF%E8%BF%9E%E6%8E%A5%E6%97%A0%E9%9C%80%E5%80%9F%E5%8A%A9-usb-%E7%BA%BF)。

### Q3 保持唤醒功能

保持唤醒功能**仅在手机充电时**有效。

### Q4 音频转发功能

使用 `Android 11` 或更高版本的设备支持音频转发，并且默认情况下启用:

- 对于 `Android 12` 或更高版本，它开箱即用。
- 对于 `Android 11` ，您需要确保在启动scrcpy时设备屏幕已解锁。假的弹出窗口将短暂出现，使系统认为shell应用程序处于前台。没有这个，音频捕获将失败。
- 对于 `Android 10` 或更早版本，无法捕获音频并自动禁用。

## 更新日志
<details>
<summary>点击查看</summary>

### Version 1.6.5
- 增加自动切换输入法功能
- 基于scrcpy v2.2(x64)
### Version 1.6.4
- 增加无线访问记录历史IP
- 修复了一些bug
### Version 1.6.3
- 增加控制器吸附scrcpy窗口
- 增加scrcpy窗口位置记忆
### Version 1.6.2
- 修复了一些bug
### Version 1.6.1
- 修复了一些bug
- 增加音频转发开关（默认启用）
### Version 1.6.0
- 基于scrcpy v2.1.1(x64)
### Version 1.5.1
- 基于scrcpy v1.25(x64)
### Version 1.5.0
- 基于scrcpy v1.21(x64)
- 优化了代码
### Version 1.4.1
- 修复了一些bug
- 增加了程序入口处的异常捕获 便于定位问题
### Version 1.4.0
- 基于scrcpy v1.19
- 增加虚拟按键（常用功能，记忆启动时位置和大小）
- 优化了代码
### Version 1.3.1
- 修复了一些bug
### Version 1.3.0
- 基于scrcpy v1.18
- 增加了一些设置项
- 修复了一些bug
### Version 1.2.0
- 基于scrcpy v1.17
- 修复了一些bug
### Version 1.1.0
- 基于scrcpy v1.16
- 增加了设置端口号功能
### Version 1.0.0
- 基于scrcpy v1.14
- 初始版本
</details>
