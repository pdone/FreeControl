using FreeControl.Utils;
using Sunny.UI;
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Windows.Forms;

namespace FreeControl
{
    public partial class Main : UIForm
    {
        public Main()
        {
            InitializeComponent();
            InitPdone();
        }

        /// <summary>
        /// 用户数据目录
        /// </summary>
        public string UserDataPath
        {
            get
            {
                return Path.Combine(System.Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "Free Control");
            }
        }

        public Setting GetUserData()
        {
            try
            {
                var tempData = new Setting();
                var fullPath = Path.Combine(UserDataPath, "config.json");
                Directory.CreateDirectory(UserDataPath);
                if (!File.Exists(fullPath))
                {
                    File.WriteAllText(fullPath, JsonHelper.json(tempData));
                }
                StreamReader reader = File.OpenText(fullPath);
                tempData = JsonHelper.jsonDes<Setting>(reader.ReadToEnd());
                reader.Close();
                return tempData;
            }
            catch
            {
                return new Setting();
            }
        }

        public void SetUserData(Setting userData)
        {
            try
            {
                var fullPath = Path.Combine(UserDataPath, "config.json");
                Directory.CreateDirectory(UserDataPath);
                File.WriteAllText(fullPath, JsonHelper.json(userData));
            }
            catch
            {

            }
        }

        Setting setting = new Setting();
        ProcessStartInfo AdbProcessInfo = null;

        public void InitPdone()
        {
            setting = GetUserData();

            Application.ApplicationExit += (sender, e) =>
            {
                SetUserData(setting);
            };

            this.FormClosed += (sender, e) =>
            {
                if (AdbProcessInfo != null)
                {
                    //退出前关闭adb
                    AdbProcessInfo.Arguments = "kill-server";
                    Process.Start(AdbProcessInfo);
                    LogHelper.Info("kill adb server");
                }
                Application.Exit();
            };
            #region 设置标题
            Assembly asm = Assembly.GetExecutingAssembly();
            FileVersionInfo fvi = FileVersionInfo.GetVersionInfo(asm.Location);
            ledTitle.Text = $"Free Control v{fvi.ProductVersion}";
            ledTitle.CharCount = 19;
            ledTitle.MouseDown += (sender, e) => DragWindow();
            this.Icon = Properties.Resources.pcm;
            #endregion

            #region 设置主题颜色
            UIStyles.SetStyle(UIStyle.Gray);

            //设置默认导航条颜色
            navTab.TabSelectedForeColor = Color.FromArgb(140, 140, 140);
            navTab.TabSelectedHighColor = Color.FromArgb(140, 140, 140);
            //设置默认导航条图标
            tabHome.ImageIndex = 0;
            tabSetting.ImageIndex = 2;
            #endregion

            #region 设置深色模式
            switchDarkMode.ValueChanged += (object sender, bool value) =>
            {
                var tabBackColor = Color.Transparent;
                if (value)
                {
                    tabBackColor = Color.FromArgb(24, 24, 24);
                    UIStyles.SetStyle(UIStyle.Black);
                    tabHome.BackColor = tabBackColor;
                    tabSetting.BackColor = tabBackColor;
                    navTab.MenuStyle = UIMenuStyle.Black;

                    btnStart.SetStyle(UIStyle.Black);

                    tabHome.ImageIndex = 1;
                    tabSetting.ImageIndex = 3;
                }
                else
                {
                    tabBackColor = Color.FromArgb(242, 242, 244);
                    UIStyles.SetStyle(UIStyle.Gray);
                    tabHome.BackColor = tabBackColor;
                    tabSetting.BackColor = tabBackColor;
                    navTab.MenuStyle = UIMenuStyle.White;

                    btnStart.SetStyle(UIStyle.Gray);

                    tabHome.ImageIndex = 0;
                    tabSetting.ImageIndex = 2;

                    navTab.TabSelectedForeColor = Color.FromArgb(140, 140, 140);
                    navTab.TabSelectedHighColor = Color.FromArgb(140, 140, 140);
                }
            };
            #endregion

            #region 切换tab事件
            navTab.SelectedIndexChanged += (object sender, EventArgs e) =>
            {


            };
            navTab.SelectTab(0);
            #endregion

            #region 设置默认值
            rbtnPx.SelectedIndex = setting.PXIndex;
            rbtnMbps.SelectedIndex = setting.BitRateIndex;
            rbtnMaxFPS.SelectedIndex = setting.MaxFPSIndex;
            switchDarkMode.Active = setting.DarkMode;
            tbxAddress.Text = setting.IPAddress;
            tbxPort.Text = setting.Port;

            #region 复选框默认值   
            cbxUseWireless.Checked = setting.UseWireless;
            cbxUseLog.Checked = setting.UseLog;
            LogHelper.enable = setting.UseLog;

            cbxCloseScreen.Checked = setting.CloseScreen;
            cbxKeepAwake.Checked = setting.KeepAwake;
            cbxAllFPS.Checked = setting.AllFPS;

            cbxHideBorder.Checked = setting.HideBorder;
            cbxFullScreen.Checked = setting.FullScreen;
            cbxTopMost.Checked = setting.TopMost;
            #endregion

            #region 参数设置事件
            rbtnPx.ValueChanged += RbtnPx_ValueChanged;
            rbtnMbps.ValueChanged += RbtnMbps_ValueChanged;
            rbtnMaxFPS.ValueChanged += RbtnMaxFPS_ValueChanged;

            cbxUseWireless.ValueChanged += CbxUseWireless_ValueChanged;
            cbxUseLog.ValueChanged += CbxUseLog_ValueChanged; ;

            switchDarkMode.ValueChanged += (sender, e) =>
            {
                setting.DarkMode = switchDarkMode.Active;
            };
            tbxAddress.TextChanged += TbxAddress_TextChanged;
            tbxPort.TextChanged += TbxPort_TextChanged;

            cbxCloseScreen.ValueChanged += CommonCbx_ValueChanged;
            cbxKeepAwake.ValueChanged += CommonCbx_ValueChanged;
            cbxAllFPS.ValueChanged += CommonCbx_ValueChanged;
            cbxHideBorder.ValueChanged += CommonCbx_ValueChanged;
            cbxFullScreen.ValueChanged += CommonCbx_ValueChanged;
            cbxTopMost.ValueChanged += CommonCbx_ValueChanged;
            #endregion

            #endregion

            #region 启动前
            string scrcpyPath = Path.Combine(UserDataPath, "scrcpy/");
            string tempFileName = "temp.zip";
            if (!Directory.Exists(scrcpyPath))
            {
                Directory.CreateDirectory(scrcpyPath);
                File.WriteAllBytes(scrcpyPath + tempFileName, Properties.Resources.scrcpy_win32_v1_14);
                if (SharpZip.UnpackFiles(scrcpyPath + tempFileName, scrcpyPath))
                {
                    File.Delete(scrcpyPath + tempFileName);
                }
            }
            #endregion

            Process scrcpy = null;

            //设置端口号命令 adb tcpip 5566
            #region 启动按钮
            btnStart.Click += (sender, e) =>
            {
                if (setting.UseWireless &&
                (string.IsNullOrWhiteSpace(setting.IPAddress) || string.IsNullOrWhiteSpace(setting.Port)))
                {
                    UIMessageTip.ShowWarning(sender as Control, "IP地址或者端口号没有填写，无法启动 -.-!", 1500);
                    return;
                }
                LogHelper.Info("starting...");//--window-title \"{ledTitle.Text}\"
                var paramlist = $" {setting.BitRate} {setting.PX} {setting.MaxFPS} {setting.OtherParam} --window-height 600";

                AdbProcessInfo = new ProcessStartInfo($@"{scrcpyPath}adb.exe",
                    $"connect {setting.IPAddress}:{setting.Port}")
                {
                    CreateNoWindow = true,//设置不在新窗口中启动新的进程        
                    UseShellExecute = false,//不使用操作系统使用的shell启动进程 
                    RedirectStandardOutput = true,//将输出信息重定向
                };

                if (setting.UseWireless)
                {
                    Process adb = Process.Start(AdbProcessInfo);
                    LogHelper.Info(adb.StandardOutput.ReadToEnd());
                    adb.WaitForExit();
                    paramlist = $"-s {setting.IPAddress}:{setting.Port} " + paramlist;
                }

                //this.Hide();
                //this.ShowInfoDialog(paramlist);
                scrcpy = Process.Start(new ProcessStartInfo($@"{scrcpyPath}scrcpy-noconsole.exe",//
                    paramlist));
                LogHelper.Info("scrcpy running...");
                scrcpy.WaitForExit();

                //this.Show();
                UIMessageTip.Show(this, "已退出", null, 1000);
            };
            #endregion
        }

        private void CbxUseLog_ValueChanged(object sender, bool value)
        {
            setting.UseLog = value;
            LogHelper.enable = value;
        }

        private void CbxUseWireless_ValueChanged(object sender, bool value)
        {
            setting.UseWireless = value;
        }

        private void TbxAddress_TextChanged(object sender, EventArgs e)
        {
            //if (!tbxAddress.Text.IsNullOrWhiteSpace())
            //{
            setting.IPAddress = tbxAddress.Text.Trim();
            //}
        }
        private void TbxPort_TextChanged(object sender, EventArgs e)
        {
            //if (!tbxPort.Text.IsNullOrWhiteSpace())
            //{
            setting.Port = tbxPort.Text.Trim();
            //}
        }

        #region 参数设置事件
        private void CommonCbx_ValueChanged(object sender, bool value)
        {
            string command = "";
            var temp = sender as UICheckBox;
            switch (temp.Text)
            {
                case "关闭屏幕":
                    setting.CloseScreen = value;
                    //关闭屏幕与镜像模式不可同时启用
                    setting.ReadOnly = !value;
                    command = setting.GetDesc("CloseScreen") + " ";
                    break;
                case "保持唤醒":
                    command = setting.GetDesc("KeepAwake") + " ";
                    setting.KeepAwake = value;
                    //保持唤醒与镜像模式不可同时启用
                    setting.ReadOnly = !value;
                    break;
                case "全帧渲染":
                    command = setting.GetDesc("AllFPS") + " ";
                    setting.AllFPS = value;
                    break;
                case "镜像模式":
                    command = setting.GetDesc("ReadOnly") + " ";
                    setting.ReadOnly = value;
                    setting.CloseScreen = !value;
                    setting.KeepAwake = !value;
                    break;
                case "隐藏边框":
                    command = setting.GetDesc("HideBorder") + " ";
                    setting.HideBorder = value;
                    break;
                case "全屏显示":
                    command = setting.GetDesc("FullScreen") + " ";
                    setting.FullScreen = value;
                    break;
                case "窗口置顶":
                    command = setting.GetDesc("TopMost") + " ";
                    setting.TopMost = value;
                    break;
            }
            LogHelper.Info(temp.Text + ":" + value);
            if (value)
            {
                setting.OtherParam += command;
            }
            else
            {
                setting.OtherParam = setting.OtherParam.Replace(command, "");
            }
        }

        private void RbtnMaxFPS_ValueChanged(object sender, int index, string text)
        {
            switch (index)
            {
                case 1:
                    setting.MaxFPS = "--max-fps 140";
                    break;
                case 2:
                    setting.MaxFPS = "--max-fps 120";
                    break;
                case 3:
                    setting.MaxFPS = "--max-fps 90";
                    break;
                case 4:
                    setting.MaxFPS = "--max-fps 60";
                    break;
                case 5:
                    setting.MaxFPS = "--max-fps 30";
                    break;
                default:
                    setting.MaxFPS = "";
                    break;
            }
            setting.MaxFPSIndex = index;
        }

        private void RbtnPx_ValueChanged(object sender, int index, string text)
        {
            switch (index)
            {
                case 1:
                    setting.PX = "-m 1920";
                    break;
                case 2:
                    setting.PX = "-m 1440";
                    break;
                case 3:
                    setting.PX = "-m 1280";
                    break;
                case 4:
                    setting.PX = "-m 960";
                    break;
                case 5:
                    setting.PX = "-m 640";
                    break;
                default:
                    setting.PX = "";
                    break;
            }
            setting.PXIndex = index;
        }

        private void RbtnMbps_ValueChanged(object sender, int index, string text)
        {
            switch (index)
            {
                case 1:
                    setting.BitRate = "-b 64M";
                    break;
                case 2:
                    setting.BitRate = "-b 32M";
                    break;
                case 3:
                    setting.BitRate = "-b 16M";
                    break;
                case 4:
                    setting.BitRate = "-b 8M";
                    break;
                case 5:
                    setting.BitRate = "-b 4M";
                    break;
                default:
                    setting.BitRate = "";
                    break;
            }
            setting.BitRateIndex = index;
        }
        #endregion

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

        private void uiLinkLabel1_LinkClicked(object sender, LinkLabelLinkClickedEventArgs e)
        {
            System.Diagnostics.Process.Start("https://baike.baidu.com/item/USB%E8%B0%83%E8%AF%95%E6%A8%A1%E5%BC%8F/5035286#2");
        }

        private void uiLabel2_Click(object sender, EventArgs e)
        {
            Form shortcut = new Form()
            {
                AutoSize = true,
                StartPosition = FormStartPosition.CenterScreen,
                FormBorderStyle = FormBorderStyle.FixedToolWindow,
            };
            shortcut.KeyPress += (senderr, ee) =>
            {
                if (ee.KeyChar == (char)Keys.Escape)
                {
                    shortcut.Close();
                }
            };
            PictureBox pictureBox = new PictureBox
            {
                Image = Properties.Resources.shortcut_zh,
                SizeMode = PictureBoxSizeMode.AutoSize,
            };
            shortcut.Controls.Add(pictureBox);
            shortcut.ShowDialog();
        }
    }

    public static class Utilitys
    {
        public static bool IsNullOrWhiteSpace(this string s)
        {
            return string.IsNullOrWhiteSpace(s);
        }

        public static string GetDesc<T>(this T obj, string name)
        {
            T ent = obj;
            var res = "";
            foreach (var item in ent.GetType().GetProperties())
            {
                if (item.Name != name)
                {
                    continue;
                }
                var v = (DescriptionAttribute[])item.GetCustomAttributes(typeof(DescriptionAttribute), false);
                if (v != null && v.Count() > 0)
                {
                    res = v[0].Description;
                    return res;
                }
            }
            return res;
        }

        public static string GetEnumDesc<T>(this T obj)
        {
            var type = obj.GetType();
            FieldInfo field = type.GetField(Enum.GetName(type, obj));
            DescriptionAttribute descAttr = Attribute.GetCustomAttribute(field, typeof(DescriptionAttribute)) as DescriptionAttribute;
            if (descAttr == null)
            {
                return string.Empty;
            }

            return descAttr.Description;
        }
    }
}
