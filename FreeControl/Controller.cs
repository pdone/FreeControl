using FreeControl.Utils;
using Sunny.UI;
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Data;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;
using System.Windows.Forms;

namespace FreeControl
{
    public partial class Controller : UIForm
    {
        public Controller()
        {
            InitializeComponent();
            TopMost = true;
            flowPanel.FlowLayoutPanel.MouseDown += (s, e) => DragWindow();
            flowPanel.FlowLayoutPanel.AutoScroll = false;

            InitButton();
            InitFormSizeAndLocation();
        }

        /// <summary>
        /// 初始化窗口大小和位置
        /// </summary>
        void InitFormSizeAndLocation()
        {
            if (Main._Setting.ControllerStartPointX == 0 && Main._Setting.ControllerStartPointY == 0)
            {
                StartPosition = FormStartPosition.CenterScreen;
            }
            else
            {
                StartPosition = FormStartPosition.Manual;
                Location = new Point(Main._Setting.ControllerStartPointX, Main._Setting.ControllerStartPointY);
            }
            if (Main._Setting.ControllerStartWidth >= 57 && Main._Setting.ControllerStartHeight >= 57)
            {
                Width = Main._Setting.ControllerStartWidth;
                Height = Main._Setting.ControllerStartHeight;
            }
            else
            {
                Width = 140;
                Height = 140;
            }
            LocationChanged += (sender, e) =>
            {
                Main._Setting.ControllerStartPointX = Location.X;
                Main._Setting.ControllerStartPointY = Location.Y;
            };
            SizeChanged += (sender, e) =>
            {
                Main._Setting.ControllerStartWidth = Width;
                Main._Setting.ControllerStartHeight = Height;
            };
            MouseDown += (s, e) => DragWindow();
        }

        /// <summary>
        /// 初始化控制器按钮
        /// </summary>
        void InitButton()
        {
            foreach (var btnName in Main._Setting.ControllerButton)
            {
                var control = this.GetControl(btnName);
                if (control is UISymbolButton)
                {
                    if (control.Name == "btnScreenshot")
                    {
                        control.Click += (sender, e) =>
                        {
                            if (ADB.Screenshot())
                            {
                                UIMessageTip.ShowOk(this, "截图成功！图片保存在桌面FreeControl Screenshot文件夹中。", 2000, false);
                            }
                        };
                    }
                    else
                    {
                        var keyCode = (KeyCode)Enum.Parse(typeof(KeyCode), btnName.Remove(0, 3));
                        control.Click += (sender, e) => ADB.ExecuteShell(ADB.GetCommand(keyCode));
                    }
                    flowPanel.AddControl(control);
                }
            }
        }

        #region 拖动窗口
        [DllImport("user32.dll")]//拖动无窗体的控件
        public static extern bool ReleaseCapture();
        [DllImport("user32.dll")]
        public static extern bool SendMessage(IntPtr hwnd, int wMsg, int wParam, int lParam);
        public const int WM_SYSCOMMAND = 0x0112;
        public const int SC_MOVE = 0xF010;
        public const int HTCAPTION = 0x0002;

        /// <summary>
        /// 拖动窗体
        /// </summary>
        public void DragWindow()
        {
            ReleaseCapture();
            SendMessage(this.Handle, WM_SYSCOMMAND, SC_MOVE + HTCAPTION, 0);
        }
        #endregion
    }
}
