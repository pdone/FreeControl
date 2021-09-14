using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading;
using System.Threading.Tasks;

namespace FreeControl.Utils
{
    public class ADB
    {
        public static Process Process;

        public static Process ShellProcess;

        public static string ADBPath;

        /// <summary>
        /// 执行adb命令
        /// </summary>
        /// <param name="command"></param>
        /// <returns></returns>
        public static string Execute(string command)
        {
            Logger.Info($"{command}", "ADB");
            var AdbProcessInfo = new ProcessStartInfo($"{ADBPath}adb.exe")
            {
                Arguments = command,
                CreateNoWindow = true,
                UseShellExecute = false,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                RedirectStandardInput = true,
                StandardOutputEncoding = Encoding.UTF8,
                StandardErrorEncoding = Encoding.UTF8,
            };
            Process = Process.Start(AdbProcessInfo);
            Process.StandardInput.WriteLine(command);
            string standardOutput = Process.StandardOutput.ReadToEnd();
            Logger.Info($"{standardOutput.Trim()}", "ADB");
            Process.WaitForExit();
            Process.Dispose();
            return standardOutput;
        }

        /// <summary>
        /// 执行截图命令
        /// </summary>
        /// <param name="command"></param>
        /// <returns></returns>
        public static bool Screenshot()
        {
            Directory.CreateDirectory(Main.ScreenshotSavePath);
            string fileName = $"\"{Main.ScreenshotSavePath}\\{DateTime.Now.ToString(Main.DatetimeFormat)}.png\"";
            string command = $"adb exec-out screencap -p > {fileName}";
            Logger.Info($"{command}", "Screenshot");
            var AdbProcessInfo = new ProcessStartInfo($"cmd.exe")
            {
                Arguments = command,
                CreateNoWindow = true,
                UseShellExecute = false,
                RedirectStandardInput = true,
            };
            using (var process = Process.Start(AdbProcessInfo))
            {
                process.StandardInput.WriteLine(command);
            }
            return true;
        }

        /// <summary>
        /// 执行adb shell命令
        /// </summary>
        /// <param name="command"></param>
        /// <returns></returns>
        public static void ExecuteShell(string command)
        {
            Logger.Info($"{command}", $"ADB Shell");

            var AdbProcessInfo = new ProcessStartInfo($"{ADBPath}adb.exe")
            {
                Arguments = "shell",
                CreateNoWindow = true,
                UseShellExecute = false,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                RedirectStandardInput = true,
                StandardOutputEncoding = Encoding.UTF8,
                StandardErrorEncoding = Encoding.UTF8,
            };
            if (ShellProcess == null)
            {
                ShellProcess = Process.Start(AdbProcessInfo);
            }
            ShellProcess.StandardInput.WriteLine(command);
        }

        /// <summary>
        /// 执行adb shell命令
        /// </summary>
        /// <param name="command"></param>
        /// <returns></returns>
        public async static void ExecuteShellAsync(string command)
        {
            Logger.Info($"{command}", "ADB Shell");

            var AdbProcessInfo = new ProcessStartInfo($"{ADBPath}adb.exe")
            {
                Arguments = "shell",
                CreateNoWindow = true,
                UseShellExecute = false,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                RedirectStandardInput = true,
                StandardOutputEncoding = Encoding.UTF8,
                StandardErrorEncoding = Encoding.UTF8,
            };
            if (ShellProcess == null)
            {
                ShellProcess = Process.Start(AdbProcessInfo);
            }
            await ShellProcess.StandardInput.WriteLineAsync(command);
        }

        /// <summary>
        /// 获取命令字符串
        /// </summary>
        /// <param name="keyCode"></param>
        /// <returns></returns>
        public static string GetCommand(KeyCode keyCode)
        {
            return $"input keyevent {(int)keyCode}";
        }

        /// <summary>
        /// 将adb所在目录加入环境变量
        /// </summary>
        /// <param name="path"></param>
        public static void AddEnvironmentPath(string path)
        {
            SysEnvironment.SetPathAfter(path);
        }
    }

    public enum KeyCode
    {
        Home = 3,//HOME 键
        Back = 4,//返回键
        Call = 5,//打开拨号应用
        HangUp = 6,//挂断电话
        VolumeUp = 24,//增加音量
        VolumeDown = 25,//降低音量
        Power = 26,//电源键
        TakePicture = 27,//拍照（需要在相机应用里）
        Browser = 64,//打开浏览器
        Menu = 82,//菜单键
        //xxxx = 85,//播放/暂停
        //xxxx = 86,//停止播放
        //xxxx = 87,//播放下一首
        //xxxx = 88,//播放上一首
        //xxxx = 122,//移动光标到行首或列表顶部
        //xxxx = 123,//移动光标到行末或列表底部
        //xxxx = 126,//恢复播放
        //xxxx = 127,//暂停播放
        Mute = 164,//静音
        //xxxx = 176,//打开系统设置
        //xxxx = 187,//切换应用
        //xxxx = 207,//打开联系人
        //xxxx = 208,//打开日历
        //xxxx = 209,//打开音乐
        //xxxx = 210,//打开计算器
        //xxxx = 220,//降低屏幕亮度
        //xxxx = 221,//提高屏幕亮度
        //xxxx = 223,//系统休眠
        //xxxx = 224,//点亮屏幕
        //xxxx = 231,//打开语音助手
        //xxxx = 276,//如果没有 wakelock 则让系统休眠
    }
}
