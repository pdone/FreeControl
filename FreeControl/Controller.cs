using FreeControl.Utils;
using Sunny.UI;
using System;
using System.Drawing;
using System.Windows.Forms;

namespace FreeControl
{
    public partial class Controller : UIForm
    {
        public Controller()
        {
            InitializeComponent();
            Icon = Properties.Resources.pcm;
            InitButton();
            InitFormSizeAndLocation();
        }

        /// <summary>
        /// 更新控制器位置
        /// </summary>
        //public void UpdateLocation()
        //{
        //    Action action = () =>
        //    {
        //        // 减去控制器自身默认宽度
        //        Location = new Point(Main._Setting.ScrcpyPointX - 57, Main._Setting.ScrcpyPointY);
        //    };
        //    Invoke(action);
        //}

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
                StartPosition = FormStartPosition.Manual;
                Location = new Point(Main._Setting.ScrcpyPointX - 57, Main._Setting.ScrcpyPointY);
            }
            else
            {
                StartPosition = FormStartPosition.CenterScreen;
            }
            SizeChanged += (sender, e) =>
            {
                Main._Setting.ControllerStartWidth = Width;
                Main._Setting.ControllerStartHeight = Height;
            };
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
    }
}
