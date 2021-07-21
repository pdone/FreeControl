# FreeControl

## 介绍
基于开源项目[**scrcpy**](https://github.com/Genymobile/scrcpy)，使用C#简单封装。

程序UI基于开源项目[**SunnyUI**](https://gitee.com/yhuse/SunnyUI)。

做本程序的目的主要是学习，另外就是自己用着方便些。

## 截图

![主界面](https://i.loli.net/2021/07/20/CGl36Nd8UvyDFw9.png)

![设置界面](https://i.loli.net/2021/07/20/rynFU1BcXjdsmZR.png)

![截图非最新版本，仅供参考](https://cdn.jsdelivr.net/gh/pdone/static@latest/img/article/free-control/3.gif)

## 版本说明
### v1.3.1
- 修复了一些bug

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
**[https://pdone.lanzoui.com/iJWO0rhslve](https://pdone.lanzoui.com/iJWO0rhslve)**

## FAQ
### Q1 输入法问题
个人测试发现，目前支持跨屏进行拼音输入的 **手机输入法APP** 如下：
- **搜狗输入法**
- **QQ输入法**
- **谷歌拼音输入法**
- **Gboard** 

此功能需要输入法APP适配，暂时只发现这4款，有更好用的输入法欢迎留言推荐。

### Q2 无线连接问题
1. 首次无线访问前必须先连接数据线；
2. 开启手机USB调试；
3. 在手机上点击信任此电脑（图1）；
4. 确保有线可以启动成功；
5. 开启手机无线调试（图2，在手机开发者模式中有无线调试的开关，小米10、小米MIX2S的MIUI系统均有此功能开关，其他手机系统以实际情况为准）；
6. 点击无线调试，跳转到（图3）所示页面，此时会看到一个无线调试的IP和端口号；
7. 将上一步获取到的IP和端口号填进去，然后点击启动按钮（电脑和手机需要在同一局域网下，并且路由器未开启AP隔离功能）。

![图1](https://i.loli.net/2021/07/20/BRXoMihKUdLrZtC.png)
![图2](https://i.loli.net/2021/07/20/I1j3PmcHsYapVKT.png)
![图3](https://i.loli.net/2021/07/20/ZXxGBdfAaF9Djoy.png)

### Q3 保持唤醒功能
保持唤醒功能仅在手机充电时有效。