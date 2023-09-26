using FreeControl.Utils;
using Sunny.UI;
using System;
using System.Diagnostics;
using System.Drawing;
using System.Runtime.InteropServices;
using System.Timers;
using System.Windows.Forms;

namespace FreeControl
{
    public partial class Controller : UIForm
    {
        [DllImport("user32.dll")]
        public static extern bool GetWindowRect(IntPtr hWnd, out Rect lpRect);

        [StructLayout(LayoutKind.Sequential)]
        public struct Rect
        {
            public int Left;
            public int Top;
            public int Right;
            public int Bottom;
        }

        /// <summary>
        /// 监听scrcpy窗体移动后的坐标
        /// </summary>
        private readonly System.Timers.Timer timer;
        /// <summary>
        /// scrcpy句柄
        /// </summary>
        private IntPtr scrcpyWindow = IntPtr.Zero;
        /// <summary>
        /// 进程名称
        /// </summary>
        private const string scrcpyWindowName = "scrcpy";
        /// <summary>
        /// 上一个有效的scrcpy坐标
        /// </summary>
        private Rect lastRect = new Rect();

        public Controller()
        {
            InitializeComponent();
            InitButton();
            InitFormSizeAndLocation();

            timer = new System.Timers.Timer
            {
                Interval = 15
            };
            timer.Elapsed += OnTimedEvent;
            timer.Enabled = true;
            Disposed += (s, e) =>
            {
                Main._Setting.ScrcpyPointX = lastRect.Left;
                Main._Setting.ScrcpyPointY = lastRect.Top;
            };
        }

        public void StopTimer()
        {
            timer.Stop();
            timer.Enabled = false;
        }

        private void OnTimedEvent(Object source, ElapsedEventArgs e)
        {
            Action action = () =>
            {
                if (scrcpyWindow == IntPtr.Zero)
                {
                    Process[] processes = Process.GetProcessesByName(scrcpyWindowName);
                    scrcpyWindow = processes[0].MainWindowHandle;
                }
                GetWindowRect(scrcpyWindow, out Rect rect);
                if (rect.Top == lastRect.Top && rect.Left == lastRect.Left)
                    return;
                if (rect.Left + rect.Top > 0)
                {
                    rect.Left += 8;
                    rect.Top += 31;
                    lastRect = rect;
                    Location = new Point(rect.Left - 57, rect.Top);
                }
            };
            if (InvokeRequired)
                Invoke(action);
        }

        /// <summary>
        /// 初始化窗口大小和位置
        /// </summary>
        void InitFormSizeAndLocation()
        {
            flowPanel.FlowLayoutPanel.AutoScroll = false;
            FormBorderStyle = FormBorderStyle.FixedSingle;
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
            if (Main._Setting.ScrcpyPointX + Main._Setting.ScrcpyPointY > 0)
            {
                // Location = new Point(Main._Setting.ControllerStartPointX, Main._Setting.ControllerStartPointY);
                StartPosition = FormStartPosition.Manual;
                Location = new Point(Main._Setting.ScrcpyPointX - 57, Main._Setting.ScrcpyPointY);
            }
            else
            {
                StartPosition = FormStartPosition.CenterScreen;
            }
            // LocationChanged += (sender, e) =>
            // {
            // Main._Setting.ControllerStartPointX = Location.X;
            // Main._Setting.ControllerStartPointY = Location.Y;
            // };
            SizeChanged += (sender, e) =>
            {
                Main._Setting.ControllerStartWidth = Width;
                Main._Setting.ControllerStartHeight = Height;
            };
            // MouseDown += (s, e) => DragWindow();
            // flowPanel.FlowLayoutPanel.MouseDown += (s, e) => DragWindow();
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
