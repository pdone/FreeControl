# FreeControl

## 介绍
基于开源项目[**scrcpy**](https://github.com/Genymobile/scrcpy)，使用C#简单封装。

程序UI基于开源项目[**SunnyUI**](https://gitee.com/yhuse/SunnyUI)。

做本程序的目的主要是学习，另外就是自己用着方便些。

## 截图
![截图非最新版本，仅供参考](https://cdn.jsdelivr.net/gh/pdone/static@latest/img/article/free-control/3.gif)

## 版本说明
### v1.3.0
- 基于scrcpy v1.18
- 增加了一些设置项
- 修复了一些bug

### v1.2.0
- 基于scrcpy v1.17
- 修复了一些bug

### v1.1.0
- 基于scrcpy v1.16
- 增加了设置端口号功能

### v1.0.0
- 基于scrcpy v1.14

### 无线连接
1. 将设备和电脑连接至同一 Wi-Fi。
2. 打开 设置 → 关于手机 → 状态信息，获取设备的 IP 地址，也可以执行以下的命令：
```
adb shell ip route | awk '{print $9}'
```
3. 点击设置端口号，正常情况下窗口中会出现一只狗头。
4. 断开设备的 USB 连接。
5. 填写IP和端口号。
6. 点击启动。

## 下载地址
> [https://pdone.lanzoui.com/iJWO0rhslve](https://pdone.lanzoui.com/iJWO0rhslve)
