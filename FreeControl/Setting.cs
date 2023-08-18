using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Text;

namespace FreeControl
{
    public class Setting
    {
        /// <summary>
        /// 其他参数
        /// </summary>
        public string OtherParam { get; set; } = "";

        /// <summary>
        /// 比特率参数
        /// </summary>
        [Description("--bit-rate")]
        public string BitRate { get; set; } = "";
        public int BitRateIndex { get; set; } = 0;
        /// <summary>
        /// 分辨率参数
        /// </summary>
        [Description("--max-size")]
        public string PX { get; set; } = "";
        public int PXIndex { get; set; } = 0;
        /// <summary>
        /// 最大帧数
        /// </summary>
        [Description("--max-fps")]
        public string MaxFPS { get; set; } = "";
        public int MaxFPSIndex { get; set; } = 0;

        /// <summary>
        /// 快捷键设置
        /// </summary>
        [Description("--shortcut-mod")]
        public string Shortcuts { get; set; } = "";
        public int ShortcutsIndex { get; set; } = 0;

        /// <summary>
        /// 关闭屏幕
        /// </summary>
        [Description("--turn-screen-off")]
        public bool CloseScreen { get; set; } = false;
        /// <summary>
        /// 保持唤醒
        /// </summary>
        [Description("--stay-awake")]
        public bool KeepAwake { get; set; } = false;
        /// <summary>
        /// 弃用 - 全帧渲染
        /// </summary>
        [Description("--render-expired-frames")]
        public bool AllFPS { get; set; } = false;
        /// <summary>
        /// 镜像模式（只读）
        /// </summary>
        [Description("--no-control")]
        public bool ReadOnly { get; set; } = false;

        /// <summary>
        /// 使用无线连接
        /// </summary>
        public bool UseWireless { get; set; } = false;
        /// <summary>
        /// IP地址
        /// </summary>
        [Description("--serial")]
        public string IPAddress { get; set; } = "";

        /// <summary>
        /// 无线ADB使用的端口号
        /// </summary>
        public string Port { get; set; } = "5555";

        /// <summary>
        /// 深色模式
        /// </summary>
        public bool DarkMode { get; set; } = false;

        /// <summary>
        /// 启用日志
        /// </summary>
        public bool UseLog { get; set; } = false;

        /// <summary>
        /// 隐藏边框
        /// </summary>
        [Description("--window-borderless")]
        public bool HideBorder { get; set; } = false;

        /// <summary>
        /// 全屏显示
        /// </summary>
        [Description("-f")]
        public bool FullScreen { get; set; } = false;

        /// <summary>
        /// 窗口置顶
        /// </summary>
        [Description("--always-on-top")]
        public bool TopMost { get; set; } = false;

        /// <summary>
        /// 显示触摸点
        /// </summary>
        [Description("--show-touches")]
        public bool ShowTouches { get; set; } = false;

        /// <summary>
        /// 窗口高度
        /// </summary>
        public int WindowHeight { get; set; } = 0;
        /// <summary>
        /// 窗口宽度
        /// </summary>
        public int WindowWidth { get; set; } = 0;

        /// <summary>
        /// 控制器是否启用
        /// </summary>
        public bool ControllerEnabled { get; set; } = false;
        /// <summary>
        /// 控制器窗体启动时X坐标
        /// </summary>
        public int ControllerStartPointX { get; set; } = 0;
        /// <summary>
        /// 控制器窗体启动时Y坐标
        /// </summary>
        public int ControllerStartPointY { get; set; } = 0;
        /// <summary>
        /// 控制器窗体启动时宽度
        /// </summary>
        public int ControllerStartWidth { get; set; } = 0;
        /// <summary>
        /// 控制器窗体启动时高度
        /// </summary>
        public int ControllerStartHeight { get; set; } = 0;

        /// <summary>
        /// 已启用的按钮名称 修改位置可进行控制器按钮排序
        /// </summary>
        public List<string> ControllerButton
        {
            get
            {
                return new List<string>()
                {
                    "btnHome",
                    "btnBack",
                    "btnMenu",
                    "btnScreenshot",
                    "btnVolumeUp",
                    "btnVolumeDown",
                    "btnMute",
                    "btnPower",
                    //"btnBrowser",
                };
            }
        }

        /// <summary>
        /// 程序版本号
        /// </summary>
        public string Version { get; set; } = "1.0.0";
    }
}
