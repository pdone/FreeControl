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
        /// 关闭屏幕
        /// </summary>
        [Description("-S")]
        public bool CloseScreen { get; set; } = false;
        /// <summary>
        /// 保持唤醒
        /// </summary>
        [Description("-w")]
        public bool KeepAwake { get; set; } = false;
        /// <summary>
        /// 全帧渲染
        /// </summary>
        [Description("--render-expired-frames")]
        public bool AllFPS { get; set; } = false;
        /// <summary>
        /// 镜像模式（只读）
        /// </summary>
        [Description("-n")]
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
    }
}
