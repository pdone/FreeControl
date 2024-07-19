using FreeControl.Utils;
using Sunny.UI;
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Reflection;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Forms;

namespace FreeControl
{
    public partial class Main : UIForm
    {
        #region 全局变量
        /// <summary>
        /// 全局配置数据
        /// </summary>
        public static Setting _Setting = new Setting();
        /// <summary>
        /// scrcpy版本
        /// </summary>
        public static readonly string ScrcpyVersion = "scrcpy-win64-v2.4";
        /// <summary>
        /// scrcpy路径
        /// </summary>
        public static readonly string ScrcpyPath = Path.Combine(UserDataPath, ScrcpyVersion + "\\");
        /// <summary>
        /// 用户桌面路径
        /// </summary>
        public static readonly string DesktopPath = Environment.GetFolderPath(Environment.SpecialFolder.Desktop);
        /// <summary>
        /// 截图保存路径
        /// </summary>
        public static readonly string ScreenshotSavePath = Path.Combine(DesktopPath, "FreeControl Screenshots");
        /// <summary>
        /// 时间格式字符串
        /// </summary>
        public static readonly string DatetimeFormat = "yyyyMMddHHmmss";
        /// <summary>
        /// 控制器
        /// </summary>
        public static Controller _Controller = null;
        /// <summary>
        /// 启动参数
        /// </summary>
        private List<string> StartParameters = new List<string>();
        /// <summary>
        /// 是否是初始化
        /// </summary>
        private bool IsInit = true;
        /// <summary>
        /// 输入法对应的包名和服务名
        /// </summary>
        readonly Dictionary<InputMethod, string> InputMethodPkgNames = new Dictionary<InputMethod, string>
        {
            { InputMethod.Sougou,"com.sohu.inputmethod.sogou/.SogouIME" },
            { InputMethod.QQPinyin,"com.tencent.qqpinyin/.QQPYInputMethodService" },
        };

        public class Info
        {
            /// <summary>
            /// 程序名称 不带空格
            /// </summary>
            public static readonly string Name = "FreeControl";
            /// <summary>
            /// 程序名称 带空格
            /// </summary>
            public static readonly string Name2 = "Free Control";
            /// <summary>
            /// scrcpy标题
            /// </summary>
            public static readonly string ScrcpyTitle = "FreeControlScrcpy";
            /// <summary>
            /// 程序名称 带版本号
            /// </summary>
            public static string NameVersion { get; set; }
        }

        /// <summary>
        /// 多语言支持
        /// </summary>
        public MultiLanguage I18n;

        public static IntPtr ControllerPtr = IntPtr.Zero;

        /// <summary>
        /// 保持后台活跃 避免界面隐藏后被自动回收
        /// </summary>
        private readonly System.Timers.Timer _Timer;
        #endregion

        #region 构造函数
        /// <summary>
        /// 构造函数
        /// </summary>
        public Main()
        {
            // 获取用户配置数据
            _Setting = GetUserData();
            // 加载语言资源
            LoadLangRes();
            // 设置语言
            string lang = _Setting.Language.GetDesc();
            I18n = new MultiLanguage(_Setting.Language);
            System.Globalization.CultureInfo UICulture = new System.Globalization.CultureInfo(lang);
            Thread.CurrentThread.CurrentUICulture = UICulture;
            // 界面保活
            _Timer = new System.Timers.Timer(_Setting.Heartbeat);
            _Timer.Elapsed += (sender, e) =>
            {
                Logger.Info("", "alive");
            };

            InitializeComponent();
            InitPdone();
            IsInit = false;
        }
        #endregion

        #region 配置文件
        /// <summary>
        /// 用户数据目录
        /// </summary>
        public static string UserDataPath
        {
            get
            {
                return Path.Combine(System.Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), Info.Name);
            }
        }
        /// <summary>
        /// 获取用户配置
        /// </summary>
        /// <returns></returns>
        public static Setting GetUserData()
        {
            try
            {
                var tempData = new Setting();
                var fullPath = Path.Combine(UserDataPath, "config.json");
                Directory.CreateDirectory(UserDataPath);
                if (!File.Exists(fullPath))
                {
                    File.WriteAllText(fullPath, JsonHelper.Obj2Str(tempData));
                }
                StreamReader reader = File.OpenText(fullPath);
                tempData = JsonHelper.Str2Obj<Setting>(reader.ReadToEnd());
                reader.Close();
                return tempData;
            }
            catch (Exception ex)
            {
                Logger.Error(ex);
                return new Setting();
            }
        }
        /// <summary>
        /// 写入用户配置
        /// </summary>
        /// <param name="userData"></param>
        public static void SetUserData(Setting userData)
        {
            try
            {
                var fullPath = Path.Combine(UserDataPath, "config.json");
                Directory.CreateDirectory(UserDataPath);
                File.WriteAllText(fullPath, JsonHelper.Obj2Str(userData));
            }
            catch (Exception ex)
            {
                Logger.Error(ex);
            }
        }
        #endregion

        #region 业务逻辑
        /// <summary>
        /// 初始化
        /// </summary>
        public void InitPdone()
        {
            // 获取程序集信息
            Assembly asm = Assembly.GetExecutingAssembly();
            FileVersionInfo fvi = FileVersionInfo.GetVersionInfo(asm.Location);
            // adb路径
            ADB.ADBPath = $@"{ScrcpyPath}";
            // 增加adb执行文件系统变量
            ADB.AddEnvironmentPath(ScrcpyPath);
            // 是否重新加载资源包
            bool reload = false;
            if (_Setting.Version != fvi.ProductVersion)
            {
                reload = true;
                _Setting.Version = fvi.ProductVersion;
            }
            // 提取资源
            ExtractResource(reload);
            if (_Setting.MainWindowX != 0 || _Setting.MainWindowY != 0)
            {
                // 避免异常坐标导致窗口无法看到 同时需要考虑双屏时的负坐标 三屏或以上暂时无法测试与兼容
                if (_Setting.MainWindowX.InRange(-1980, 1980) && _Setting.MainWindowY.InRange(-1080, 1080))
                {
                    // 主窗口位置
                    StartPosition = FormStartPosition.Manual;
                    Location = new Point(_Setting.MainWindowX, _Setting.MainWindowY);
                }
            }

            #region 控件状态
            uiLabel7.Visible = _Setting.EnableSwitchIME;
            linkIME.Visible = _Setting.EnableSwitchIME;
            #endregion

            #region 事件绑定
            // 退出时保存用户配置数据
            Application.ApplicationExit += (sender, e) =>
            {
                SetUserData(_Setting);
                ADB.Execute("kill-server");
            };
            FormClosed += (sender, e) => Application.Exit();
            // 窗口拖动
            MouseDown += (sender, e) => Extend.DragWindow(Handle);
            // 窗口位置改变
            LocationChanged += (sender, e) =>
            {
                _Setting.MainWindowX = Location.X;
                _Setting.MainWindowY = Location.Y;
            };
            ledTitle.MouseDown += (sender, e) => Extend.DragWindow(Handle);
            tabHome.MouseDown += (sender, e) => Extend.DragWindow(Handle);
            tabSetting.MouseDown += (sender, e) => Extend.DragWindow(Handle);
            uiLabel1.MouseDown += (sender, e) => Extend.DragWindow(Handle);
            // 关闭按钮和最小化按钮
            btnClose.Click += (sender, e) => Close();
            btnMini.Click += (sender, e) => WindowState = FormWindowState.Minimized;
            // 启动按钮
            btnStart.Click += StartButtonClick;
            // 深色模式切换
            switchDarkMode.ValueChanged += SwitchDarkMode_ValueChanged;
            // scrcpy窗口大小设置
            updownHeight.ValueChanged += (sender, e) => _Setting.WindowHeight = updownHeight.Value;
            updownWidth.ValueChanged += (sender, e) => _Setting.WindowWidth = updownWidth.Value;
            rbtnShortcuts.ValueChanged += RbtnShortcuts_ValueChanged;
            // 下拉框
            comboPx.SelectedValueChanged += ComboPx_SelectedValueChanged;
            comboMbps.SelectedValueChanged += ComboMbps_SelectedValueChanged;
            comboMaxFPS.SelectedValueChanged += ComboMaxFPS_SelectedValueChanged;
            comboIp.SelectedIndexChanged += (sender, e) => tbxIp.Text = comboIp.SelectedItem as string;
            LoadHistoryIPs();
            // 文本框
            tbxIp.TextChanged += TbxAddress_TextChanged;
            tbxPort.TextChanged += TbxPort_TextChanged;
            // CheckBox
            cbxUseWireless.ValueChanged += CbxUseWireless_ValueChanged;
            cbxUseLog.ValueChanged += CbxUseLog_ValueChanged;
            cbxControllerEnabled.ValueChanged += CbxControllerEnabled_ValueChanged;
            cbxCloseScreen.ValueChanged += CommonCbx_ValueChanged;
            cbxKeepAwake.ValueChanged += CommonCbx_ValueChanged;
            cbxHideBorder.ValueChanged += CommonCbx_ValueChanged;
            cbxFullScreen.ValueChanged += CommonCbx_ValueChanged;
            cbxTopMost.ValueChanged += CommonCbx_ValueChanged;
            cbxShowTouches.ValueChanged += CommonCbx_ValueChanged;
            cbxReadOnly.ValueChanged += CommonCbx_ValueChanged;
            cbxAudioEnabled.ValueChanged += cbxAudioEnabled_ValueChanged;

            uiLabel4.DoubleClick += (sender, e) => Process.Start(Logger.path);
            uiLabel3.DoubleClick += (sender, e) => Process.Start(UserDataPath);

            #endregion

            #region 设置标题和图标
            Info.NameVersion = $"FreeControl v{fvi.ProductVersion}";
            Text = Info.NameVersion;
            ledTitle.Text = Info.NameVersion;
            ledTitle.CharCount = 19;
            Icon = Properties.Resources.pcm;
            #endregion

            #region 设置主题颜色
            UIStyles.SetStyle(UIStyle.Gray);
            // 设置默认导航条颜色
            navTab.TabSelectedForeColor = Color.FromArgb(140, 140, 140);
            navTab.TabSelectedHighColor = Color.FromArgb(140, 140, 140);
            // 设置默认导航条图标
            tabHome.ImageIndex = 0;
            tabSetting.ImageIndex = 2;
            this.BackColor = Color.FromArgb(140, 140, 140);
            navTab.TabBackColor = Color.FromArgb(222, 222, 222);
            #endregion

            #region 切换tab事件
            // navTab.SelectTab(0);
            #endregion

            #region 配置项默认值
            comboPx.Items[0] = I18n.def;
            comboMbps.Items[0] = I18n.def;
            comboMaxFPS.Items[0] = I18n.def;
            comboPx.SelectedIndex = _Setting.PXIndex;
            comboMbps.SelectedIndex = _Setting.BitRateIndex;
            comboMaxFPS.SelectedIndex = _Setting.MaxFPSIndex;
            rbtnShortcuts.SelectedIndex = _Setting.ShortcutsIndex;
            switchDarkMode.Active = _Setting.DarkMode;
            tbxIp.Text = _Setting.IPAddress;
            comboIp.Text = _Setting.IPAddress;
            tbxPort.Text = _Setting.Port;
            updownHeight.Value = _Setting.WindowHeight;
            updownWidth.Value = _Setting.WindowWidth;
            Logger.enable = _Setting.UseLog;
            cbxUseWireless.Checked = _Setting.UseWireless;
            cbxUseLog.Checked = _Setting.UseLog;
            cbxControllerEnabled.Checked = _Setting.ControllerEnabled;
            cbxCloseScreen.Checked = _Setting.CloseScreen;
            cbxKeepAwake.Checked = _Setting.KeepAwake;
            cbxHideBorder.Checked = _Setting.HideBorder;
            cbxFullScreen.Checked = _Setting.FullScreen;
            cbxTopMost.Checked = _Setting.TopMost;
            cbxShowTouches.Checked = _Setting.ShowTouches;
            cbxReadOnly.Checked = _Setting.ReadOnly;
            cbxAudioEnabled.Checked = _Setting.AudioEnabled;
            linkIME.Text = I18n.imes[(InputMethod)_Setting.IME];
            tbxIp.Watermark = I18n.tbxIpPlaceholder;
            tbxPort.Watermark = I18n.tbxPortPlaceholder;
            #endregion
        }

        /// <summary>
        /// 提取内置资源
        /// </summary>
        private void ExtractResource(bool reload = false)
        {
            string tempFileName = "temp.zip";
            // 如果重新加载 且旧目录存在 删除后重新解压资源
            if (reload && Directory.Exists(ScrcpyPath))
            {
                Directory.Delete(ScrcpyPath, true);
            }
            if (!Directory.Exists(ScrcpyPath))
            {
                Directory.CreateDirectory(ScrcpyPath);
                File.WriteAllBytes(ScrcpyPath + tempFileName, Properties.Resources.scrcpy_win64_v2_4);
                // 解压缩
                ZipFile.ExtractToDirectory(ScrcpyPath + tempFileName, UserDataPath);
                // 解压完成删除压缩包
                File.Delete(ScrcpyPath + tempFileName);
            }
        }

        /// <summary>
        /// 加载语言资源 此资源控制界面布局
        /// </summary>
        public void LoadLangRes()
        {
            if (_Setting.Language == Lang.en)
            {
                string dirResEn = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "en");
                Directory.CreateDirectory(dirResEn);
                string path = Path.Combine(dirResEn, "FreeControl.resources.dll");
                if (File.Exists(path) == false)
                {
                    // 英文资源文件
                    File.WriteAllBytes(path, Properties.Resources.en_FreeControl_resources);
                }
            }
        }

        /// <summary>
        /// 启动按钮
        /// </summary>
        /// <param name="sender"></param>
        /// <param name="e"></param>
        private void StartButtonClick(object sender, EventArgs e)
        {
            try
            {
                Task.Run(() => ButtonHandle(true));

                if (_Setting.UseWireless &&
                (string.IsNullOrWhiteSpace(_Setting.IPAddress) || string.IsNullOrWhiteSpace(_Setting.Port)))
                {
                    ShowMessage(I18n.msgIpNull);
                    return;
                }

                Logger.Info("starting...");
                StartParameters.Clear();
                StartParameters.Add(_Setting.BitRate);
                StartParameters.Add(_Setting.MaxFPS);
                StartParameters.Add(_Setting.Shortcuts);
                StartParameters.Add(_Setting.PX);
                // 设置屏幕高度 800
                if (_Setting.WindowHeight > 0)
                {
                    StartParameters.Add($"--window-height {_Setting.WindowHeight}");
                }
                if (_Setting.WindowWidth > 0)
                {
                    StartParameters.Add($"--window-width {_Setting.WindowWidth}");
                }
                // 设置scrcpy启动位置
                if (_Setting.ScrcpyPointX + _Setting.ScrcpyPointY > 0)
                {
                    StartParameters.Add($"--window-x={_Setting.ScrcpyPointX} --window-y={_Setting.ScrcpyPointY}");
                }
                // 设置标题
                StartParameters.Add($"--window-title \"{Info.ScrcpyTitle}\"");
                // 设置断开后锁定屏幕
                if (_Setting.PowerOffOnClose)
                {
                    StartParameters.Add("--power-off-on-close");
                }
                StartParameters.Add(_Setting.CustomArgs);
                if (_Setting.AudioEnabled == false) StartParameters.Add(_Setting.GetDesc("AudioEnabled"));// 不转发音频

                // 其他参数
                if (_Setting.CloseScreen) StartParameters.Add(_Setting.GetDesc("CloseScreen"));
                if (_Setting.KeepAwake) StartParameters.Add(_Setting.GetDesc("KeepAwake"));
                if (_Setting.ReadOnly) StartParameters.Add(_Setting.GetDesc("ReadOnly"));
                if (_Setting.HideBorder) StartParameters.Add(_Setting.GetDesc("HideBorder"));
                if (_Setting.FullScreen) StartParameters.Add(_Setting.GetDesc("FullScreen"));
                if (_Setting.TopMost) StartParameters.Add(_Setting.GetDesc("TopMost"));
                if (_Setting.ShowTouches) StartParameters.Add(_Setting.GetDesc("ShowTouches"));

                // 无线访问
                if (_Setting.UseWireless)
                {
                    StartParameters.Add($"--tcpip={_Setting.IPAddress}:{_Setting.Port}");
                    ADBConnectFunc.BeginInvoke(ADBConnectCallback, ADBConnectFunc);
                }
                else
                {
                    StartParameters.Add("--select-usb");// 使用usb
                    RunScrcpy();
                }
            }
            catch (Exception ex)
            {
                Logger.Error(ex);
            }
        }

        /// <summary>
        /// ADB连接委托
        /// </summary>
        private readonly Func<string> ADBConnectFunc = () =>
        {
            // 不再验证adb连接状态
            // return ADB.Execute($"connect {_Setting.IPAddress}:{_Setting.Port}");
            return string.Empty;
        };

        /// <summary>
        /// ADB连接回调
        /// </summary>
        /// <param name="ar"></param>
        private void ADBConnectCallback(IAsyncResult ar)
        {
            Func<string> tempFunc = ar.AsyncState as Func<string>;
            string result = tempFunc.EndInvoke(ar);
            // ADB连接返回消息不为空
            if (result.IsNotNull() && result.Contains("cannot connect"))
            {
                ButtonHandle(false);
                ShowMessage(result);
                return;
            }
            if (_Setting.IPAddress.IsNotNull()
                && !_Setting.HistoryIPs.Contains(_Setting.IPAddress))
            {
                _Setting.HistoryIPs.Add(_Setting.IPAddress);
            }
            RunScrcpy();
        }

        /// <summary>
        /// 运行Scrcpy
        /// </summary>
        private void RunScrcpy()
        {
            SetUserData(_Setting);// 启动前保存一下配置文件
            Logger.Info("scrcpy running...");
            string args = "";
            StartParameters.ForEach(x =>
            {
                if (x.IsNotNull())
                {
                    args += x + " ";
                }
            });
            var processStartInfo = new ProcessStartInfo($@"{ScrcpyPath}scrcpy.exe")
            {
                Arguments = args,
                CreateNoWindow = true,
                UseShellExecute = false,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                RedirectStandardInput = true,
            };
            Logger.Info(args);
            Process scrcpy = Process.Start(processStartInfo);
            scrcpy.EnableRaisingEvents = true;

            // 监听Scrcpy窗口移动
            MoveListener.StartListening(scrcpy, !_Setting.UseWireless);

            var tempOutput = "";
            scrcpy.OutputDataReceived += (ss, ee) =>
            {
                if (ee.Data.IsNotNull())
                {
                    Logger.Info($"{ee.Data}", "scrcpy");
                    tempOutput = ee.Data;
                    if (tempOutput.Contains("INFO: Device:"))
                    {
                        FromHandle(true);
                    }
                }
            };

            void exitHandle()
            {
                SetUserData(_Setting);// 关闭scrcpy后保存一下配置文件
                if (_Setting.EnableSwitchIME && _Setting.IME != 0 && _Setting.IMEOrigin.IsNotNull())
                {
                    ADB.Execute($"shell ime set {_Setting.IMEOrigin}");
                }
                MoveListener.StopListening();
                FromHandle(false);
                ButtonHandle(false);
                LoadHistoryIPs(true);
                ShowMessage(I18n.msgExit);
            }

            scrcpy.ErrorDataReceived += (ss, ee) =>
            {
                if (ee.Data.IsNotNull())
                {
                    Logger.Info($"{ee.Data}", "scrcpy");
                    if (ee.Data.Contains("ERROR"))
                    {
                        exitHandle();
                    }
                }
            };
            scrcpy.Exited += (ss, ee) => exitHandle();
            scrcpy.BeginErrorReadLine();
            scrcpy.BeginOutputReadLine();

            if (_Setting.EnableSwitchIME && _Setting.IME != 0)
            {
                // 获取当前输入法
                string strCurIME = ADB.Execute($"adb shell settings get secure default_input_method");
                if (strCurIME.IsNotNull())
                {
                    _Setting.IMEOrigin = strCurIME;
                }
                // 启动时切换到该输入法
                ADB.Execute($"shell ime set {InputMethodPkgNames[(InputMethod)_Setting.IME]}");
            }
        }
        /// <summary>
        /// 显示提示消息
        /// </summary>
        /// <param name="content"></param>
        /// <param name="delay"></param>
        /// <param name="isFloat"></param>
        private void ShowMessage(string content, int delay = 1500, bool isFloat = true)
        {
            Action action = () =>
            {
                UIMessageTip.Show(this, content, null, delay, isFloat);
            };
            Invoke(action);
        }

        /// <summary>
        /// 按钮可用性控制
        /// </summary>
        /// <param name="isStart"></param>
        private void ButtonHandle(bool isStart)
        {
            Action action = () =>
            {
                if (isStart)
                {
                    btnStart.Enabled = false;
                    btnStart.Text = I18n.btnStarting;
                }
                else
                {
                    btnStart.Enabled = true;
                    btnStart.Text = I18n.btnStartDef;
                }
            };
            Invoke(action);
        }

        /// <summary>
        /// 控制器可用性控制
        /// </summary>
        /// <param name="isStart"></param>
        private void FromHandle(bool isStart)
        {
            Action action = () =>
            {
                if (isStart)
                {
                    Hide();
                    // 是否启用控制器
                    if (_Setting.ControllerEnabled)
                    {
                        _Controller = new Controller();
                        ControllerPtr = _Controller.Handle;
                        _Controller.Show();
                    }
                    _Timer?.Start();
                }
                else
                {
                    _Controller?.Dispose();
                    Show();
                    Activate();
                    Focus();
                    _Timer?.Stop();
                }
            };
            Invoke(action);
        }

        /// <summary>
        /// 加载历史IP
        /// </summary>
        /// <param name="isReload"></param>
        void LoadHistoryIPs(bool isReload = false)
        {
            Action action = () =>
            {
                comboIp.Items.Clear();
                _Setting.HistoryIPs.ForEach(ip =>
                {
                    comboIp.Items.Add(ip);
                });
            };
            if (isReload)
                Invoke(action);
            else
                action();
        }
        #endregion

        #region 配置项改变事件
        /// <summary>
        /// 主题模式切换
        /// </summary>
        /// <param name="sender"></param>
        /// <param name="value"></param>
        private void SwitchDarkMode_ValueChanged(object sender, bool value)
        {
            UpdateStyle(value);
            _Setting.DarkMode = switchDarkMode.Active;
        }

        void UpdateStyle(bool isDark)
        {
            Color tabBackColor;
            Color foreColor;
            UIStyle curStyle;
            if (isDark)
            {
                tabBackColor = Color.FromArgb(24, 24, 24);
                foreColor = Color.FromArgb(192, 192, 192);
                curStyle = UIStyle.Black;
                UIStyles.SetStyle(curStyle);
                navTab.MenuStyle = UIMenuStyle.Black;

                tabHome.ImageIndex = 1;
                tabSetting.ImageIndex = 3;

                btnStart.FillHoverColor = Color.FromArgb(20, 54, 94);
                btnStart.FillDisableColor = tabBackColor;
                btnStart.FillPressColor = Color.FromArgb(20, 54, 94);
                btnStart.ForeHoverColor = Color.White;
                btnStart.ForeDisableColor = Color.White;
                btnStart.ForePressColor = Color.White;
            }
            else
            {
                tabBackColor = Color.FromArgb(242, 242, 244);
                foreColor = Color.FromArgb(48, 48, 48);
                curStyle = UIStyle.Gray;
                UIStyles.SetStyle(curStyle);
                navTab.MenuStyle = UIMenuStyle.White;

                tabHome.ImageIndex = 0;
                tabSetting.ImageIndex = 2;

                navTab.TabSelectedColor = tabBackColor;
                navTab.TabSelectedForeColor = Color.FromArgb(140, 140, 140);
                navTab.TabSelectedHighColor = Color.FromArgb(140, 140, 140);
                navTab.TabBackColor = Color.FromArgb(222, 222, 222);
            }

            tabHome.BackColor = tabBackColor;
            tabSetting.BackColor = tabBackColor;


            tbxPort.FillColor = tabBackColor;
            tbxIp.FillColor = tabBackColor;

            comboPx.FillColor = tabBackColor;
            comboMbps.FillColor = tabBackColor;
            comboMaxFPS.FillColor = tabBackColor;
            comboIp.FillColor = tabBackColor;



            tbxPort.ForeColor = foreColor;
            tbxIp.ForeColor = foreColor;
            comboPx.ForeColor = foreColor;
            comboMbps.ForeColor = foreColor;
            comboMaxFPS.ForeColor = foreColor;
            comboIp.ForeColor = foreColor;


            BackColor = Color.FromArgb(140, 140, 140);
            btnClose.Style = UIStyle.Gray;
            btnClose.ForeColor = Color.FromArgb(250, 240, 230);
            btnMini.Style = UIStyle.Gray;
            btnMini.ForeColor = Color.FromArgb(250, 240, 230);
        }

        private void ComboMaxFPS_SelectedValueChanged(object sender, EventArgs e)
        {
            int index = comboMaxFPS.SelectedIndex;
            switch (index)
            {
                case 1:
                    _Setting.MaxFPS = "--max-fps=140";
                    break;
                case 2:
                    _Setting.MaxFPS = "--max-fps=120";
                    break;
                case 3:
                    _Setting.MaxFPS = "--max-fps=90";
                    break;
                case 4:
                    _Setting.MaxFPS = "--max-fps=60";
                    break;
                case 5:
                    _Setting.MaxFPS = "--max-fps=30";
                    break;
                default:
                    _Setting.MaxFPS = "";
                    break;
            }
            _Setting.MaxFPSIndex = index;
        }

        private void ComboMbps_SelectedValueChanged(object sender, EventArgs e)
        {
            int index = comboMbps.SelectedIndex;
            switch (index)
            {
                case 1:
                    _Setting.BitRate = "-b 64M";
                    break;
                case 2:
                    _Setting.BitRate = "-b 32M";
                    break;
                case 3:
                    _Setting.BitRate = "-b 16M";
                    break;
                case 4:
                    _Setting.BitRate = "-b 8M";
                    break;
                case 5:
                    _Setting.BitRate = "-b 4M";
                    break;
                case 6:
                    _Setting.BitRate = "-b 1M";
                    break;
                default:
                    _Setting.BitRate = "";
                    break;
            }
            _Setting.BitRateIndex = index;
        }

        private void ComboPx_SelectedValueChanged(object sender, EventArgs e)
        {
            int index = comboPx.SelectedIndex;
            switch (index)
            {
                case 1:
                    _Setting.PX = "-m 1920";
                    break;
                case 2:
                    _Setting.PX = "-m 1440";
                    break;
                case 3:
                    _Setting.PX = "-m 1280";
                    break;
                case 4:
                    _Setting.PX = "-m 960";
                    break;
                case 5:
                    _Setting.PX = "-m 640";
                    break;
                default:
                    _Setting.PX = "";
                    break;
            }
            _Setting.PXIndex = index;
        }

        private void CbxControllerEnabled_ValueChanged(object sender, bool value)
        {
            _Setting.ControllerEnabled = value;
        }

        private void CbxUseLog_ValueChanged(object sender, bool value)
        {
            _Setting.UseLog = value;
            Logger.enable = value;
        }

        private void CbxUseWireless_ValueChanged(object sender, bool value)
        {
            _Setting.UseWireless = value;
        }

        private void TbxAddress_TextChanged(object sender, EventArgs e)
        {
            _Setting.IPAddress = tbxIp.Text.Trim();
        }

        private void TbxPort_TextChanged(object sender, EventArgs e)
        {
            _Setting.Port = tbxPort.Text.Trim();
        }

        private void CommonCbx_ValueChanged(object sender, bool value)
        {
            var temp = sender as UICheckBox;
            switch (temp.Text)
            {
                case "关闭屏幕":
                case "Close Screen":
                    _Setting.CloseScreen = value;
                    if (value)
                    {
                        // 关闭屏幕与只读模式不可同时启用
                        _Setting.ReadOnly = false;
                        cbxReadOnly.Checked = false;
                        // UIMessageTip.ShowWarning(this, "勾选关闭屏幕后，将取消只读模式！", 1500);
                    }
                    break;
                case "保持唤醒":
                case "Stay Awake":
                    _Setting.KeepAwake = value;
                    if (value)
                    {
                        // 保持唤醒与只读模式不可同时启用
                        _Setting.ReadOnly = false;
                        cbxReadOnly.Checked = false;
                        // UIMessageTip.ShowWarning(this, "勾选保持唤醒后，将取消只读模式！", 1500);
                    }
                    break;
                case "只读模式":
                case "Read Only":
                    _Setting.ReadOnly = value;
                    if (value)
                    {
                        _Setting.CloseScreen = false;
                        cbxCloseScreen.Checked = false;
                        _Setting.KeepAwake = false;
                        cbxKeepAwake.Checked = false;
                        // UIMessageTip.ShowWarning(this, "勾选只读模式后，将取消关闭屏幕和保持唤醒！", 1500);
                    }
                    break;
                case "隐藏边框":
                case "Hide Border":
                    _Setting.HideBorder = value;
                    break;
                case "全屏显示":
                case "Full Screen":
                    _Setting.FullScreen = value;
                    break;
                case "窗口置顶":
                case "Top Most":
                    _Setting.TopMost = value;
                    break;
                case "显示触摸":
                case "Show Touch":
                    _Setting.ShowTouches = value;
                    break;
            }
            if (IsInit == false)// 初始化时 不记录此处日志
                Logger.Info(temp.Text + ":" + value);
        }

        private void RbtnShortcuts_ValueChanged(object sender, int index, string text)
        {
            switch (index)
            {
                case 1:
                    _Setting.Shortcuts = "lctrl+lalt";
                    break;
                case 2:
                    _Setting.Shortcuts = "lalt";
                    break;
                default:
                    _Setting.Shortcuts = "lctrl";
                    break;
            }
            _Setting.Shortcuts = $"{_Setting.GetDesc("Shortcuts")}={_Setting.Shortcuts}";
            _Setting.ShortcutsIndex = index;
        }

        private void cbxAudioEnabled_ValueChanged(object sender, bool value)
        {
            _Setting.AudioEnabled = value;
        }
        #endregion

        #region 文本跳转链接设置
        private void linkEnabledADB_Click(object sender, EventArgs e)
        {
            System.Diagnostics.Process.Start("https://baike.baidu.com/item/USB%E8%B0%83%E8%AF%95%E6%A8%A1%E5%BC%8F/5035286#2");
        }

        private void linkSetPort_Click(object sender, EventArgs e)
        {
            if (UIMessageBox.Show(I18n.linkSetPort, I18n.linkSetPortTitle,
                _Setting.DarkMode ? UIStyle.Black : UIStyle.Gray, UIMessageBoxButtons.OKCancel, false))
            {
                var batPath = ScrcpyPath + "SetProt.bat";
                if (!File.Exists(batPath))
                {
                    // 提取嵌入资源
                    FileHelper.ExtractResFile("FreeControl.SetProt.bat", batPath);
                }
                if (File.Exists(batPath))
                {
                    System.Diagnostics.Process.Start(batPath, ScrcpyVersion);
                }
            }
        }

        private void linkIssues_Click(object sender, EventArgs e)
        {
            System.Diagnostics.Process.Start("https://github.com/pdone/FreeControl/issues");
        }

        private void lbAllShortcut_Click(object sender, EventArgs e)
        {
            Form dlgShortcut = new Form()
            {
                AutoSize = true,
                StartPosition = FormStartPosition.CenterScreen,
                FormBorderStyle = FormBorderStyle.FixedToolWindow,
                MinimizeBox = false,
                MaximizeBox = false,
            };
            dlgShortcut.KeyPress += (senderr, ee) =>
            {
                if (ee.KeyChar == (char)Keys.Escape)
                {
                    dlgShortcut.Close();
                }
            };
            Image img = null;
            dlgShortcut.Icon = Properties.Resources.pcm;
            if (_Setting.Language == Lang.zh_cn)
            {
                if (_Setting.DarkMode)
                    img = Properties.Resources.shortcut_zh_dark;
                else
                    img = Properties.Resources.shortcut_zh;
            }
            else if (_Setting.Language == Lang.en)
            {
                if (_Setting.DarkMode)
                    img = Properties.Resources.shortcut_en_dark;
                else
                    img = Properties.Resources.shortcut_en;
            }

            PictureBox pictureBox = new PictureBox
            {
                Image = img,
                SizeMode = PictureBoxSizeMode.AutoSize,
            };
            pictureBox.MouseDown += (ss, ee) => Extend.DragWindow(dlgShortcut.Handle);
            pictureBox.BorderStyle = BorderStyle.None;
            pictureBox.Margin = new Padding(0);
            dlgShortcut.Controls.Add(pictureBox);
            dlgShortcut.ShowDialog();
        }

        private void linkIME_Click(object sender, EventArgs e)
        {
            var list = I18n.imes.Values.ToList();
            int select = _Setting.IME;
            if (UISelectDialog.ShowSelectDialog(this, ref select, list, I18n.linkImeTitle, I18n.linkImeContent))
            {
                _Setting.IME = select;
                linkIME.Text = I18n.imes[(InputMethod)_Setting.IME];
            }
        }

        private void linkLang_Click(object sender, EventArgs e)
        {
            var list = I18n.langs;
            int select = (int)_Setting.Language;
            if (UISelectDialog.ShowSelectDialog(this, ref select, list, I18n.linkLangTitle, I18n.linkLangContent))
            {
                if (select == (int)_Setting.Language)
                {
                    return;
                }
                _Setting.Language = (Lang)select;
                LoadLangRes();

                System.Windows.Forms.Application.Restart();
            }
        }
        #endregion
    }

    #region 语言相关
    /// <summary>
    /// 输入法
    /// </summary>
    public enum InputMethod
    {
        Unknown = 0,
        Sougou,
        QQPinyin,
    }

    /// <summary>
    /// 多语言
    /// </summary>
    public enum Lang
    {
        [Description("zh-CN")]
        zh_cn = 0,
        [Description("en")]
        en,
    }

    public class MultiLanguage
    {
        public MultiLanguage(Lang lang)
        {
            switch (lang)
            {
                case Lang.en:
                    // 选择输入法弹窗
                    linkImeTitle = "Select";
                    linkImeContent = "After startup, it will switch to the selected input method";
                    linkLangTitle = "Select";
                    linkLangContent = " ";
                    linkSetPort =
                        "1. Turn on USB debug mode\n" +
                        "2. Connect your phone to your computer\n" +
                        "3. I will use adb tcpip 5555\n" +
                        "4. If you see a dog's head, it's a success";
                    linkSetPortTitle = "Tip";

                    tbxIpPlaceholder = "IP Address";
                    tbxPortPlaceholder = "Port";

                    btnStartDef = "Start";
                    btnStarting = "Starting...";
                    msgExit = "Exit";
                    msgIpNull = "IP address or port must input -.-!";

                    unset = "Unset";
                    def = "Default";

                    imes.Clear();
                    imes.Add(InputMethod.Unknown, unset);
                    imes.Add(InputMethod.Sougou, "Sougou IME");
                    imes.Add(InputMethod.QQPinyin, "QQ IME");

                    UILocalize.OK = "OK";
                    UILocalize.Cancel = "Cancel";

                    break;

                case Lang.zh_cn:
                default:
                    break;
            }
        }

        public string linkImeTitle = "选择";
        public string linkImeContent = "启动后将切换到所选输入法，退出程序时自动还原";
        public string linkLangTitle = "选择";
        public string linkLangContent = " ";
        public string linkSetPort =
            "1、使用数据线将手机连接电脑\n" +
            "2、手机开启调试模式\n" +
            "3、程序将使用adb tcpip 5555命令修改无线调试端口号\n" +
            "4、点击确定后若看到一只狗头，则表示设置端口号成功";
        public string linkSetPortTitle = "提示";

        public string tbxIpPlaceholder = "IP 地址";
        public string tbxPortPlaceholder = "端口号";

        public string btnStartDef = "启动";
        public string btnStarting = "正在启动...";
        public string msgExit = "已退出";
        public string msgIpNull = "IP地址或者端口号没有填写，无法启动 -.-!";

        public string unset = "未设置";
        public string def = "默认";

        public Dictionary<InputMethod, string> imes = new Dictionary<InputMethod, string>()
        {
            { InputMethod.Unknown, "未设置" },
            { InputMethod.Sougou, "搜狗输入法" },
            { InputMethod.QQPinyin, "QQ输入法" },
        };

        public List<string> langs = new List<string>()
        {
            "简体中文",
            "English",
        };
    }
    #endregion
}
