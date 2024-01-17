# FreeControl

[![](https://img.shields.io/github/license/pdone/FreeControl?style=for-the-badge)](https://github.com/pdone/FreeControl/blob/master/LICENSE)
[![](https://img.shields.io/github/release/pdone/FreeControl?style=for-the-badge)](https://github.com/pdone/FreeControl/releases/latest)
[![](https://img.shields.io/github/downloads/pdone/FreeControl/total?style=for-the-badge)](https://github.com/pdone/FreeControl/releases)
[![](https://img.shields.io/github/stars/pdone/FreeControl?style=for-the-badge)](https://github.com/pdone/FreeControl)
[![](https://img.shields.io/github/issues/pdone/FreeControl?style=for-the-badge)](https://github.com/pdone/FreeControl/issues)
[![](https://img.shields.io/badge/site-Pdone's_Blog-blue?style=for-the-badge)](https://awaw.cc)

## Introduction

[ English | [中文](https://github.com/pdone/FreeControl/blob/master/README.md) ]

Based on the open source project [**scrcpy**](https://github.com/Genymobile/scrcpy), it provides a simple interactive interface.

- Programming Language `C#`
- Development Tools `Visual Studio 2022`
- Runtime `.NET Framework 4.7.2`

## Interface

![](https://raw.githubusercontent.com/pdone/static/master/img/article/free-control/1.6.8_1_en.png)

![](https://raw.githubusercontent.com/pdone/static/master/img/article/free-control/1.6.8_2_en.png)

![](https://raw.githubusercontent.com/pdone/static/master/img/article/free-control/v1.4.0_5.gif)

## Download
### GitHub Release

https://github.com/pdone/FreeControl/releases/latest/download/FreeControl.exe

### My Proxy

https://cdn.awaw.cc/gh/pdone/FreeControl/releases/latest/download/FreeControl.exe

## Code Repository

[![](https://img.shields.io/badge/github-Free_Control-blue?style=for-the-badge&logo=github)](https://github.com/pdone/FreeControl)

## Update Record

[![](https://img.shields.io/badge/updete-record-fedcba?style=for-the-badge)](https://github.com/pdone/FreeControl/blob/master/FreeControl/Update.en.md)

## FAQ

### Input Method Issues

My personal testing found that currently the phone input method apps that support cross-screen input are as follows:

- Sogou IME
- QQ IME
- Google Pinyin IME
- Gboard
- WeChat IME

This feature requires input method apps to be adapted. If you have any better suggestions, please feel free to recommend them in the comments.

### Device Connection Issues

#### USB connection

USB connection normal use adb by the need to ensure that:

1. Hardware status is normal.

   Including Android devices in the normal power state, USB cable and interface intact.

2. Android devices and USB debugging mode is on.

   You can go to the "Settings" - "Developer options" - "Android Debug" view.

   If you can not find the developer options in the settings, it needs to make it through an egg is displayed: In the "Settings" - "About phone" continuous click "version number" 7 times.

3. The device driver is normal.

   It seems to worry about the Linux and Mac OS X, the Windows likely to be encountered in the case of the need to install drivers, this can be confirmed right "Computer" - "Properties", the "Device Manager" in view on related equipment Is there a yellow exclamation point or question mark, if not explain the driving state has been good. Otherwise, you can download a mobile assistant class program to install the driver first.

4. Status after confirmation via USB cable connected computers and devices.

   ```sh
   adb devices
   ```

   If you can see

   ```sh
   6d56e83a device
   ```

   Description Connection successful.

#### Wireless connection (Android11+)

[Doc in Android developers](https://developer.android.com/studio/command-line/adb#connect-to-a-device-over-wi-fi-android-11+)

Android 11 and higher support deploying and debugging your app wirelessly from your workstation using Android Debug Bridge (adb). For example, you can deploy your debuggable app to multiple remote devices without physically connecting your device via USB. This eliminates the need to deal with common USB connection issues, such as driver installation.

To use wireless debugging, you need to pair your device to your workstation using a pairing code. Your workstation and device must be connected to the same wireless network. To connect to your device, follow these steps:

1. Update to the latest version of the [SDK Platform-Tools](https://developer.android.com/studio/releases/platform-tools).

2. Connect Android device to run adb computer connected to the same local area network, such as connected to the same WiFi.

3. Enable the **Wireless debugging** option.

4. On the dialog that asks **Allow wireless debugging on this network?**, click **Allow**.

5. Select **Pair device with pairing code**. Take note of the pairing code, IP address, and port number displayed on the device.

6. On your workstation, open a terminal and navigate to `android_sdk/platform-tools`.

7. Run `adb pair ipaddr:port`. Use the IP address and port number from step 5.

8. When prompted, enter the pairing code that you received in step 5. A message indicates that your device has been successfully paired.

   ```sh
   none
   Enter pairing code: xxxxxx
   Successfully paired to ...
   ```

9. (For Linux or Microsoft Windows only) Run `adb connect ipaddr:port`. Use the IP address and port under **Wireless debugging**.

#### Wireless connection (need to use the USB cable)

In addition to the USB connection to the computer to use adb, can also be a wireless connection - although the connection process is also step using USB needs, but after a successful connection to your device can get rid of the limit of the USB cable within a certain range it !

Steps:

1. Connect Android device to run adb computer connected to the same local area network, such as connected to the same WiFi.

2. The device connected to the computer via a USB cable.

   Make sure the connection is successful (you can run `adb devices` see if you can list the device).

3. Allow the device listens on port 5555 TCP / IP connections:

   ```sh
   adb tcpip 5555
   ```

4. Disconnect the USB connection.

5. Find the IP address of the device.

   Generally the 'Settings' in - "About phone" - "state information" - "IP address" is found.

6. Connect the device via IP address.

   ```sh
   adb connect <device-ip-address>
   ```

   Here `<device-ip-address>` is the IP address of the device found in the previous step.

7. Confirm the connection status.

   ```sh
   adb devices
   ```

   If you can see

   ```sh
   <device-ip-address>:5555 device
   ```

   Description Connection successful.

If you can not connect, verify that Android devices and the computer is connected to the same WiFi, then execute `adb connect <device-ip-address>` that step again;

If that does not work, by `adb kill-server` restart the adb and then try it all over again.

#### Wireless connection (without using the USB cable)

**Need root privileges**, not detailed here, see more [click here](https://github.com/mzlogin/awesome-adb/blob/master/README.en.md#wireless-connection-without-using-the-usb-cable)。

### Keep Wake-up Function

Only works when the phone is charging.

### Audio Forwarding Function

Audio forwarding is supported on devices using 'Android 11 'or higher and is enabled by default:

- For `Android 12` or higher, it works out of the box.
- For `Android 11` , you need to make sure that the device screen is unlocked when starting scrcpy. The fake pop-up window will appear briefly, making the system think that the shell application is in the foreground. Without this, audio capture will fail.
- For `Android 10` or earlier, audio cannot be captured and is automatically disabled.

## Donate

If you think this project is helpful, please invite the author to have a cup of coffee.☕

[![Donate](https://img.shields.io/badge/Donate-PayPal-blue.svg?style=for-the-badge)](https://paypal.me/alexpdone)

## Stargazers Over Time
[![Stargazers over time](https://starchart.cc/pdone/FreeControl.svg?variant=adaptive)](https://starchart.cc/pdone/FreeControl)
