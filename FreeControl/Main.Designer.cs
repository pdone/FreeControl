namespace FreeControl
{
    partial class Main
    {
        /// <summary>
        /// 必需的设计器变量。
        /// </summary>
        private System.ComponentModel.IContainer components = null;

        /// <summary>
        /// 清理所有正在使用的资源。
        /// </summary>
        /// <param name="disposing">如果应释放托管资源，为 true；否则为 false。</param>
        protected override void Dispose(bool disposing)
        {
            if (disposing && (components != null))
            {
                components.Dispose();
            }
            base.Dispose(disposing);
        }

        #region Windows 窗体设计器生成的代码

        /// <summary>
        /// 设计器支持所需的方法 - 不要修改
        /// 使用代码编辑器修改此方法的内容。
        /// </summary>
        private void InitializeComponent()
        {
            this.components = new System.ComponentModel.Container();
            System.ComponentModel.ComponentResourceManager resources = new System.ComponentModel.ComponentResourceManager(typeof(Main));
            this.ledTitle = new Sunny.UI.UILedDisplay();
            this.navImgList = new System.Windows.Forms.ImageList(this.components);
            this.navTab = new Sunny.UI.UITabControlMenu();
            this.tabHome = new System.Windows.Forms.TabPage();
            this.uiLabel2 = new Sunny.UI.UILabel();
            this.uiLinkLabel1 = new Sunny.UI.UILinkLabel();
            this.uiLabel1 = new Sunny.UI.UILabel();
            this.btnStart = new Sunny.UI.UISymbolButton();
            this.tabSetting = new System.Windows.Forms.TabPage();
            this.tbxPort = new Sunny.UI.UITextBox();
            this.cbxOtherSetting = new Sunny.UI.UIGroupBox();
            this.cbxUseLog = new Sunny.UI.UICheckBox();
            this.cbxAllFPS = new Sunny.UI.UICheckBox();
            this.cbxKeepAwake = new Sunny.UI.UICheckBox();
            this.cbxCloseScreen = new Sunny.UI.UICheckBox();
            this.cbxUseWireless = new Sunny.UI.UICheckBox();
            this.rbtnPx = new Sunny.UI.UIRadioButtonGroup();
            this.tbxAddress = new Sunny.UI.UITextBox();
            this.uDarkMode = new Sunny.UI.UILabel();
            this.rbtnMaxFPS = new Sunny.UI.UIRadioButtonGroup();
            this.switchDarkMode = new Sunny.UI.UISwitch();
            this.rbtnMbps = new Sunny.UI.UIRadioButtonGroup();
            this.cbxHideBorder = new Sunny.UI.UICheckBox();
            this.cbxFullScreen = new Sunny.UI.UICheckBox();
            this.cbxTopMost = new Sunny.UI.UICheckBox();
            this.navTab.SuspendLayout();
            this.tabHome.SuspendLayout();
            this.tabSetting.SuspendLayout();
            this.cbxOtherSetting.SuspendLayout();
            this.SuspendLayout();
            // 
            // ledTitle
            // 
            this.ledTitle.BackColor = System.Drawing.Color.Transparent;
            this.ledTitle.BorderColor = System.Drawing.Color.Transparent;
            this.ledTitle.BorderInColor = System.Drawing.Color.Transparent;
            this.ledTitle.CharCount = 12;
            this.ledTitle.ForeColor = System.Drawing.Color.Linen;
            this.ledTitle.IntervalH = 0;
            this.ledTitle.IntervalV = 0;
            this.ledTitle.LedBackColor = System.Drawing.Color.Transparent;
            this.ledTitle.Location = new System.Drawing.Point(3, 35);
            this.ledTitle.Name = "ledTitle";
            this.ledTitle.Size = new System.Drawing.Size(222, 24);
            this.ledTitle.TabIndex = 0;
            this.ledTitle.Text = "Free Control";
            // 
            // navImgList
            // 
            this.navImgList.ImageStream = ((System.Windows.Forms.ImageListStreamer)(resources.GetObject("navImgList.ImageStream")));
            this.navImgList.TransparentColor = System.Drawing.Color.Transparent;
            this.navImgList.Images.SetKeyName(0, "Home.png");
            this.navImgList.Images.SetKeyName(1, "Home_Dark.png");
            this.navImgList.Images.SetKeyName(2, "Setting.png");
            this.navImgList.Images.SetKeyName(3, "Setting_Dark.png");
            // 
            // navTab
            // 
            this.navTab.Alignment = System.Windows.Forms.TabAlignment.Left;
            this.navTab.Controls.Add(this.tabHome);
            this.navTab.Controls.Add(this.tabSetting);
            this.navTab.Dock = System.Windows.Forms.DockStyle.Fill;
            this.navTab.DrawMode = System.Windows.Forms.TabDrawMode.OwnerDrawFixed;
            this.navTab.Font = new System.Drawing.Font("微软雅黑", 12F);
            this.navTab.ImageList = this.navImgList;
            this.navTab.ItemSize = new System.Drawing.Size(50, 50);
            this.navTab.Location = new System.Drawing.Point(0, 63);
            this.navTab.MenuStyle = Sunny.UI.UIMenuStyle.Custom;
            this.navTab.Multiline = true;
            this.navTab.Name = "navTab";
            this.navTab.SelectedIndex = 0;
            this.navTab.Size = new System.Drawing.Size(658, 277);
            this.navTab.SizeMode = System.Windows.Forms.TabSizeMode.Fixed;
            this.navTab.Style = Sunny.UI.UIStyle.Custom;
            this.navTab.TabBackColor = System.Drawing.Color.FromArgb(((int)(((byte)(250)))), ((int)(((byte)(250)))), ((int)(((byte)(250)))));
            this.navTab.TabIndex = 6;
            this.navTab.TabSelectedColor = System.Drawing.Color.FromArgb(((int)(((byte)(240)))), ((int)(((byte)(240)))), ((int)(((byte)(240)))));
            this.navTab.TabUnSelectedForeColor = System.Drawing.Color.FromArgb(((int)(((byte)(48)))), ((int)(((byte)(48)))), ((int)(((byte)(48)))));
            // 
            // tabHome
            // 
            this.tabHome.BackColor = System.Drawing.Color.Transparent;
            this.tabHome.Controls.Add(this.uiLabel2);
            this.tabHome.Controls.Add(this.uiLinkLabel1);
            this.tabHome.Controls.Add(this.uiLabel1);
            this.tabHome.Controls.Add(this.btnStart);
            this.tabHome.ImageIndex = 0;
            this.tabHome.Location = new System.Drawing.Point(51, 0);
            this.tabHome.Name = "tabHome";
            this.tabHome.Size = new System.Drawing.Size(607, 277);
            this.tabHome.TabIndex = 0;
            // 
            // uiLabel2
            // 
            this.uiLabel2.AutoSize = true;
            this.uiLabel2.BackColor = System.Drawing.Color.Transparent;
            this.uiLabel2.Cursor = System.Windows.Forms.Cursors.Hand;
            this.uiLabel2.Font = new System.Drawing.Font("微软雅黑", 10F);
            this.uiLabel2.ForeColor = System.Drawing.SystemColors.ActiveCaptionText;
            this.uiLabel2.Location = new System.Drawing.Point(515, 243);
            this.uiLabel2.Name = "uiLabel2";
            this.uiLabel2.Size = new System.Drawing.Size(79, 20);
            this.uiLabel2.Style = Sunny.UI.UIStyle.Custom;
            this.uiLabel2.TabIndex = 3;
            this.uiLabel2.Text = "查看快捷键";
            this.uiLabel2.TextAlign = System.Drawing.ContentAlignment.MiddleLeft;
            this.uiLabel2.Click += new System.EventHandler(this.uiLabel2_Click);
            // 
            // uiLinkLabel1
            // 
            this.uiLinkLabel1.ActiveLinkColor = System.Drawing.Color.FromArgb(((int)(((byte)(220)))), ((int)(((byte)(155)))), ((int)(((byte)(40)))));
            this.uiLinkLabel1.AutoSize = true;
            this.uiLinkLabel1.Font = new System.Drawing.Font("微软雅黑", 9F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.Point, ((byte)(134)));
            this.uiLinkLabel1.ForeColor = System.Drawing.Color.FromArgb(((int)(((byte)(48)))), ((int)(((byte)(48)))), ((int)(((byte)(48)))));
            this.uiLinkLabel1.LinkArea = new System.Windows.Forms.LinkArea(0, 12);
            this.uiLinkLabel1.LinkBehavior = System.Windows.Forms.LinkBehavior.HoverUnderline;
            this.uiLinkLabel1.LinkColor = System.Drawing.Color.FromArgb(((int)(((byte)(48)))), ((int)(((byte)(48)))), ((int)(((byte)(48)))));
            this.uiLinkLabel1.Location = new System.Drawing.Point(259, 125);
            this.uiLinkLabel1.Name = "uiLinkLabel1";
            this.uiLinkLabel1.Size = new System.Drawing.Size(124, 17);
            this.uiLinkLabel1.Style = Sunny.UI.UIStyle.Custom;
            this.uiLinkLabel1.TabIndex = 2;
            this.uiLinkLabel1.TabStop = true;
            this.uiLinkLabel1.Text = "(开启USB调试的方法)";
            this.uiLinkLabel1.VisitedLinkColor = System.Drawing.Color.FromArgb(((int)(((byte)(230)))), ((int)(((byte)(80)))), ((int)(((byte)(80)))));
            this.uiLinkLabel1.LinkClicked += new System.Windows.Forms.LinkLabelLinkClickedEventHandler(this.uiLinkLabel1_LinkClicked);
            // 
            // uiLabel1
            // 
            this.uiLabel1.AutoSize = true;
            this.uiLabel1.Font = new System.Drawing.Font("微软雅黑", 10F);
            this.uiLabel1.ForeColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.uiLabel1.Location = new System.Drawing.Point(118, 123);
            this.uiLabel1.Name = "uiLabel1";
            this.uiLabel1.Size = new System.Drawing.Size(358, 140);
            this.uiLabel1.Style = Sunny.UI.UIStyle.Custom;
            this.uiLabel1.StyleCustomMode = true;
            this.uiLabel1.TabIndex = 1;
            this.uiLabel1.Text = "1、开启USB调试模式\r\n2、使用数据线将手机连接到电脑上\r\n3、点击启动按钮\r\n\r\n首次连接会弹出是否信任该电脑，点击始终信任即可\r\n无线访问需要在设置页面勾选" +
    "启用，并且填写正确IP地址\r\n若点击启动后没反应，请尝试手动点亮一次手机屏幕";
            this.uiLabel1.TextAlign = System.Drawing.ContentAlignment.MiddleLeft;
            // 
            // btnStart
            // 
            this.btnStart.Cursor = System.Windows.Forms.Cursors.Hand;
            this.btnStart.FillColor = System.Drawing.Color.FromArgb(((int)(((byte)(15)))), ((int)(((byte)(40)))), ((int)(((byte)(70)))));
            this.btnStart.FillHoverColor = System.Drawing.Color.FromArgb(((int)(((byte)(216)))), ((int)(((byte)(233)))), ((int)(((byte)(255)))));
            this.btnStart.FillPressColor = System.Drawing.Color.FromArgb(((int)(((byte)(235)))), ((int)(((byte)(243)))), ((int)(((byte)(255)))));
            this.btnStart.Font = new System.Drawing.Font("微软雅黑", 12F);
            this.btnStart.ForeHoverColor = System.Drawing.Color.FromArgb(((int)(((byte)(130)))), ((int)(((byte)(130)))), ((int)(((byte)(130)))));
            this.btnStart.ForePressColor = System.Drawing.Color.FromArgb(((int)(((byte)(130)))), ((int)(((byte)(130)))), ((int)(((byte)(130)))));
            this.btnStart.Location = new System.Drawing.Point(224, 57);
            this.btnStart.Name = "btnStart";
            this.btnStart.Padding = new System.Windows.Forms.Padding(28, 0, 0, 0);
            this.btnStart.RectColor = System.Drawing.Color.FromArgb(((int)(((byte)(130)))), ((int)(((byte)(130)))), ((int)(((byte)(130)))));
            this.btnStart.RectHoverColor = System.Drawing.Color.FromArgb(((int)(((byte)(130)))), ((int)(((byte)(130)))), ((int)(((byte)(130)))));
            this.btnStart.RectPressColor = System.Drawing.Color.FromArgb(((int)(((byte)(130)))), ((int)(((byte)(130)))), ((int)(((byte)(130)))));
            this.btnStart.Size = new System.Drawing.Size(145, 51);
            this.btnStart.Style = Sunny.UI.UIStyle.Custom;
            this.btnStart.Symbol = 61671;
            this.btnStart.TabIndex = 0;
            this.btnStart.Text = "启动";
            // 
            // tabSetting
            // 
            this.tabSetting.BackColor = System.Drawing.Color.Transparent;
            this.tabSetting.Controls.Add(this.tbxPort);
            this.tabSetting.Controls.Add(this.cbxOtherSetting);
            this.tabSetting.Controls.Add(this.cbxUseWireless);
            this.tabSetting.Controls.Add(this.rbtnPx);
            this.tabSetting.Controls.Add(this.tbxAddress);
            this.tabSetting.Controls.Add(this.uDarkMode);
            this.tabSetting.Controls.Add(this.rbtnMaxFPS);
            this.tabSetting.Controls.Add(this.switchDarkMode);
            this.tabSetting.Controls.Add(this.rbtnMbps);
            this.tabSetting.ImageIndex = 2;
            this.tabSetting.Location = new System.Drawing.Point(51, 0);
            this.tabSetting.Name = "tabSetting";
            this.tabSetting.Size = new System.Drawing.Size(607, 277);
            this.tabSetting.TabIndex = 1;
            // 
            // tbxPort
            // 
            this.tbxPort.Cursor = System.Windows.Forms.Cursors.IBeam;
            this.tbxPort.FillColor = System.Drawing.Color.White;
            this.tbxPort.Font = new System.Drawing.Font("微软雅黑", 11F);
            this.tbxPort.Location = new System.Drawing.Point(259, 240);
            this.tbxPort.Margin = new System.Windows.Forms.Padding(4, 5, 4, 5);
            this.tbxPort.Maximum = 2147483647D;
            this.tbxPort.Minimum = -2147483648D;
            this.tbxPort.Name = "tbxPort";
            this.tbxPort.Padding = new System.Windows.Forms.Padding(5);
            this.tbxPort.RectColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.tbxPort.Size = new System.Drawing.Size(67, 27);
            this.tbxPort.Style = Sunny.UI.UIStyle.Custom;
            this.tbxPort.StyleCustomMode = true;
            this.tbxPort.TabIndex = 23;
            this.tbxPort.Watermark = "端口号";
            // 
            // cbxOtherSetting
            // 
            this.cbxOtherSetting.Controls.Add(this.cbxTopMost);
            this.cbxOtherSetting.Controls.Add(this.cbxFullScreen);
            this.cbxOtherSetting.Controls.Add(this.cbxHideBorder);
            this.cbxOtherSetting.Controls.Add(this.cbxUseLog);
            this.cbxOtherSetting.Controls.Add(this.cbxAllFPS);
            this.cbxOtherSetting.Controls.Add(this.cbxKeepAwake);
            this.cbxOtherSetting.Controls.Add(this.cbxCloseScreen);
            this.cbxOtherSetting.FillColor = System.Drawing.Color.FromArgb(((int)(((byte)(242)))), ((int)(((byte)(242)))), ((int)(((byte)(244)))));
            this.cbxOtherSetting.Font = new System.Drawing.Font("微软雅黑", 11F);
            this.cbxOtherSetting.Location = new System.Drawing.Point(9, 120);
            this.cbxOtherSetting.Margin = new System.Windows.Forms.Padding(4, 5, 4, 5);
            this.cbxOtherSetting.Name = "cbxOtherSetting";
            this.cbxOtherSetting.Padding = new System.Windows.Forms.Padding(0, 32, 0, 0);
            this.cbxOtherSetting.RectColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.cbxOtherSetting.Size = new System.Drawing.Size(586, 110);
            this.cbxOtherSetting.Style = Sunny.UI.UIStyle.Custom;
            this.cbxOtherSetting.TabIndex = 3;
            this.cbxOtherSetting.Text = "其他设置";
            // 
            // cbxUseLog
            // 
            this.cbxUseLog.Cursor = System.Windows.Forms.Cursors.Hand;
            this.cbxUseLog.Font = new System.Drawing.Font("微软雅黑", 11F);
            this.cbxUseLog.Location = new System.Drawing.Point(305, 26);
            this.cbxUseLog.Name = "cbxUseLog";
            this.cbxUseLog.Padding = new System.Windows.Forms.Padding(22, 0, 0, 0);
            this.cbxUseLog.Size = new System.Drawing.Size(92, 29);
            this.cbxUseLog.Style = Sunny.UI.UIStyle.Custom;
            this.cbxUseLog.TabIndex = 3;
            this.cbxUseLog.Text = "程序日志";
            // 
            // cbxAllFPS
            // 
            this.cbxAllFPS.Cursor = System.Windows.Forms.Cursors.Hand;
            this.cbxAllFPS.Font = new System.Drawing.Font("微软雅黑", 11F);
            this.cbxAllFPS.Location = new System.Drawing.Point(207, 26);
            this.cbxAllFPS.Name = "cbxAllFPS";
            this.cbxAllFPS.Padding = new System.Windows.Forms.Padding(22, 0, 0, 0);
            this.cbxAllFPS.Size = new System.Drawing.Size(92, 29);
            this.cbxAllFPS.Style = Sunny.UI.UIStyle.Custom;
            this.cbxAllFPS.TabIndex = 2;
            this.cbxAllFPS.Text = "全帧模式";
            // 
            // cbxKeepAwake
            // 
            this.cbxKeepAwake.Cursor = System.Windows.Forms.Cursors.Hand;
            this.cbxKeepAwake.Font = new System.Drawing.Font("微软雅黑", 11F);
            this.cbxKeepAwake.Location = new System.Drawing.Point(109, 26);
            this.cbxKeepAwake.Name = "cbxKeepAwake";
            this.cbxKeepAwake.Padding = new System.Windows.Forms.Padding(22, 0, 0, 0);
            this.cbxKeepAwake.Size = new System.Drawing.Size(92, 29);
            this.cbxKeepAwake.Style = Sunny.UI.UIStyle.Custom;
            this.cbxKeepAwake.TabIndex = 1;
            this.cbxKeepAwake.Text = "保持唤醒";
            // 
            // cbxCloseScreen
            // 
            this.cbxCloseScreen.Cursor = System.Windows.Forms.Cursors.Hand;
            this.cbxCloseScreen.Font = new System.Drawing.Font("微软雅黑", 11F);
            this.cbxCloseScreen.Location = new System.Drawing.Point(11, 26);
            this.cbxCloseScreen.Name = "cbxCloseScreen";
            this.cbxCloseScreen.Padding = new System.Windows.Forms.Padding(22, 0, 0, 0);
            this.cbxCloseScreen.Size = new System.Drawing.Size(92, 29);
            this.cbxCloseScreen.Style = Sunny.UI.UIStyle.Custom;
            this.cbxCloseScreen.TabIndex = 0;
            this.cbxCloseScreen.Text = "关闭屏幕";
            // 
            // cbxUseWireless
            // 
            this.cbxUseWireless.CheckBoxColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.cbxUseWireless.Cursor = System.Windows.Forms.Cursors.Hand;
            this.cbxUseWireless.Font = new System.Drawing.Font("微软雅黑", 11F);
            this.cbxUseWireless.Location = new System.Drawing.Point(9, 240);
            this.cbxUseWireless.Name = "cbxUseWireless";
            this.cbxUseWireless.Padding = new System.Windows.Forms.Padding(22, 0, 0, 0);
            this.cbxUseWireless.Size = new System.Drawing.Size(85, 29);
            this.cbxUseWireless.Style = Sunny.UI.UIStyle.Custom;
            this.cbxUseWireless.TabIndex = 23;
            this.cbxUseWireless.Text = "无线访问";
            // 
            // rbtnPx
            // 
            this.rbtnPx.BackColor = System.Drawing.Color.Transparent;
            this.rbtnPx.ColumnCount = 2;
            this.rbtnPx.FillColor = System.Drawing.Color.FromArgb(((int)(((byte)(242)))), ((int)(((byte)(242)))), ((int)(((byte)(244)))));
            this.rbtnPx.Font = new System.Drawing.Font("微软雅黑", 11F);
            this.rbtnPx.Items.AddRange(new object[] {
            "默认",
            "1920",
            "1440",
            "1280",
            "960",
            "640"});
            this.rbtnPx.ItemSize = new System.Drawing.Size(85, 27);
            this.rbtnPx.Location = new System.Drawing.Point(9, 5);
            this.rbtnPx.Margin = new System.Windows.Forms.Padding(4, 5, 4, 5);
            this.rbtnPx.Name = "rbtnPx";
            this.rbtnPx.Padding = new System.Windows.Forms.Padding(0, 32, 0, 0);
            this.rbtnPx.Radius = 10;
            this.rbtnPx.RectColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.rbtnPx.Size = new System.Drawing.Size(190, 114);
            this.rbtnPx.Style = Sunny.UI.UIStyle.Custom;
            this.rbtnPx.TabIndex = 18;
            this.rbtnPx.Text = "分辨率（垂直）";
            // 
            // tbxAddress
            // 
            this.tbxAddress.Cursor = System.Windows.Forms.Cursors.IBeam;
            this.tbxAddress.FillColor = System.Drawing.Color.White;
            this.tbxAddress.Font = new System.Drawing.Font("微软雅黑", 11F);
            this.tbxAddress.Location = new System.Drawing.Point(101, 240);
            this.tbxAddress.Margin = new System.Windows.Forms.Padding(4, 5, 4, 5);
            this.tbxAddress.Maximum = 2147483647D;
            this.tbxAddress.Minimum = -2147483648D;
            this.tbxAddress.Name = "tbxAddress";
            this.tbxAddress.Padding = new System.Windows.Forms.Padding(5);
            this.tbxAddress.RectColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.tbxAddress.Size = new System.Drawing.Size(150, 27);
            this.tbxAddress.Style = Sunny.UI.UIStyle.Custom;
            this.tbxAddress.StyleCustomMode = true;
            this.tbxAddress.TabIndex = 22;
            this.tbxAddress.Watermark = "输入IP地址";
            // 
            // uDarkMode
            // 
            this.uDarkMode.AutoSize = true;
            this.uDarkMode.Font = new System.Drawing.Font("微软雅黑", 11F);
            this.uDarkMode.Location = new System.Drawing.Point(440, 243);
            this.uDarkMode.Name = "uDarkMode";
            this.uDarkMode.Size = new System.Drawing.Size(69, 20);
            this.uDarkMode.Style = Sunny.UI.UIStyle.Custom;
            this.uDarkMode.TabIndex = 16;
            this.uDarkMode.Text = "深色模式";
            this.uDarkMode.TextAlign = System.Drawing.ContentAlignment.MiddleLeft;
            // 
            // rbtnMaxFPS
            // 
            this.rbtnMaxFPS.BackColor = System.Drawing.Color.Transparent;
            this.rbtnMaxFPS.ColumnCount = 2;
            this.rbtnMaxFPS.FillColor = System.Drawing.Color.FromArgb(((int)(((byte)(242)))), ((int)(((byte)(242)))), ((int)(((byte)(244)))));
            this.rbtnMaxFPS.Font = new System.Drawing.Font("微软雅黑", 11F);
            this.rbtnMaxFPS.Items.AddRange(new object[] {
            "默认",
            "140",
            "120",
            "90",
            "60",
            "30"});
            this.rbtnMaxFPS.ItemSize = new System.Drawing.Size(85, 27);
            this.rbtnMaxFPS.Location = new System.Drawing.Point(405, 5);
            this.rbtnMaxFPS.Margin = new System.Windows.Forms.Padding(4, 5, 4, 5);
            this.rbtnMaxFPS.Name = "rbtnMaxFPS";
            this.rbtnMaxFPS.Padding = new System.Windows.Forms.Padding(0, 32, 0, 0);
            this.rbtnMaxFPS.Radius = 10;
            this.rbtnMaxFPS.RectColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.rbtnMaxFPS.Size = new System.Drawing.Size(190, 114);
            this.rbtnMaxFPS.Style = Sunny.UI.UIStyle.Custom;
            this.rbtnMaxFPS.TabIndex = 21;
            this.rbtnMaxFPS.Text = "帧数限制";
            // 
            // switchDarkMode
            // 
            this.switchDarkMode.ActiveText = "ON";
            this.switchDarkMode.Cursor = System.Windows.Forms.Cursors.Hand;
            this.switchDarkMode.Font = new System.Drawing.Font("微软雅黑", 11F);
            this.switchDarkMode.InActiveText = "OFF";
            this.switchDarkMode.Location = new System.Drawing.Point(520, 240);
            this.switchDarkMode.Name = "switchDarkMode";
            this.switchDarkMode.Size = new System.Drawing.Size(75, 29);
            this.switchDarkMode.Style = Sunny.UI.UIStyle.Custom;
            this.switchDarkMode.TabIndex = 17;
            // 
            // rbtnMbps
            // 
            this.rbtnMbps.BackColor = System.Drawing.Color.Transparent;
            this.rbtnMbps.ColumnCount = 2;
            this.rbtnMbps.FillColor = System.Drawing.Color.FromArgb(((int)(((byte)(242)))), ((int)(((byte)(242)))), ((int)(((byte)(244)))));
            this.rbtnMbps.Font = new System.Drawing.Font("微软雅黑", 11F);
            this.rbtnMbps.Items.AddRange(new object[] {
            "默认",
            "64Mbps",
            "32Mbps",
            "16Mbps",
            "8Mbps",
            "4Mbps"});
            this.rbtnMbps.ItemSize = new System.Drawing.Size(85, 27);
            this.rbtnMbps.Location = new System.Drawing.Point(207, 5);
            this.rbtnMbps.Margin = new System.Windows.Forms.Padding(4, 5, 4, 5);
            this.rbtnMbps.Name = "rbtnMbps";
            this.rbtnMbps.Padding = new System.Windows.Forms.Padding(0, 32, 0, 0);
            this.rbtnMbps.Radius = 10;
            this.rbtnMbps.RectColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.rbtnMbps.Size = new System.Drawing.Size(190, 114);
            this.rbtnMbps.Style = Sunny.UI.UIStyle.Custom;
            this.rbtnMbps.TabIndex = 19;
            this.rbtnMbps.Text = "比特率";
            // 
            // cbxHideBorder
            // 
            this.cbxHideBorder.Cursor = System.Windows.Forms.Cursors.Hand;
            this.cbxHideBorder.Font = new System.Drawing.Font("微软雅黑", 11F);
            this.cbxHideBorder.Location = new System.Drawing.Point(403, 26);
            this.cbxHideBorder.Name = "cbxHideBorder";
            this.cbxHideBorder.Padding = new System.Windows.Forms.Padding(22, 0, 0, 0);
            this.cbxHideBorder.Size = new System.Drawing.Size(92, 29);
            this.cbxHideBorder.Style = Sunny.UI.UIStyle.Custom;
            this.cbxHideBorder.TabIndex = 4;
            this.cbxHideBorder.Text = "隐藏边框";
            // 
            // cbxFullScreen
            // 
            this.cbxFullScreen.Cursor = System.Windows.Forms.Cursors.Hand;
            this.cbxFullScreen.Font = new System.Drawing.Font("微软雅黑", 11F);
            this.cbxFullScreen.Location = new System.Drawing.Point(11, 61);
            this.cbxFullScreen.Name = "cbxFullScreen";
            this.cbxFullScreen.Padding = new System.Windows.Forms.Padding(22, 0, 0, 0);
            this.cbxFullScreen.Size = new System.Drawing.Size(92, 29);
            this.cbxFullScreen.Style = Sunny.UI.UIStyle.Custom;
            this.cbxFullScreen.TabIndex = 5;
            this.cbxFullScreen.Text = "全屏显示";
            // 
            // cbxTopMost
            // 
            this.cbxTopMost.Cursor = System.Windows.Forms.Cursors.Hand;
            this.cbxTopMost.Font = new System.Drawing.Font("微软雅黑", 11F);
            this.cbxTopMost.Location = new System.Drawing.Point(109, 61);
            this.cbxTopMost.Name = "cbxTopMost";
            this.cbxTopMost.Padding = new System.Windows.Forms.Padding(22, 0, 0, 0);
            this.cbxTopMost.Size = new System.Drawing.Size(92, 29);
            this.cbxTopMost.Style = Sunny.UI.UIStyle.Custom;
            this.cbxTopMost.TabIndex = 6;
            this.cbxTopMost.Text = "窗口置顶";
            // 
            // Main
            // 
            this.AutoScaleDimensions = new System.Drawing.SizeF(96F, 96F);
            this.AutoScaleMode = System.Windows.Forms.AutoScaleMode.Dpi;
            this.ClientSize = new System.Drawing.Size(658, 340);
            this.Controls.Add(this.navTab);
            this.Controls.Add(this.ledTitle);
            this.Icon = ((System.Drawing.Icon)(resources.GetObject("$this.Icon")));
            this.MaximizeBox = false;
            this.Name = "Main";
            this.Padding = new System.Windows.Forms.Padding(0, 63, 0, 0);
            this.RectColor = System.Drawing.Color.FromArgb(((int)(((byte)(130)))), ((int)(((byte)(130)))), ((int)(((byte)(130)))));
            this.Style = Sunny.UI.UIStyle.Custom;
            this.Text = "";
            this.TitleColor = System.Drawing.Color.FromArgb(((int)(((byte)(130)))), ((int)(((byte)(130)))), ((int)(((byte)(130)))));
            this.TitleHeight = 63;
            this.navTab.ResumeLayout(false);
            this.tabHome.ResumeLayout(false);
            this.tabHome.PerformLayout();
            this.tabSetting.ResumeLayout(false);
            this.tabSetting.PerformLayout();
            this.cbxOtherSetting.ResumeLayout(false);
            this.ResumeLayout(false);

        }

        #endregion
        private Sunny.UI.UILedDisplay ledTitle;
        private System.Windows.Forms.ImageList navImgList;
        private Sunny.UI.UITabControlMenu navTab;
        private System.Windows.Forms.TabPage tabHome;
        private System.Windows.Forms.TabPage tabSetting;
        private Sunny.UI.UISymbolButton btnStart;
        private Sunny.UI.UICheckBox cbxUseWireless;
        private Sunny.UI.UIRadioButtonGroup rbtnPx;
        private Sunny.UI.UITextBox tbxAddress;
        private Sunny.UI.UILabel uDarkMode;
        private Sunny.UI.UIRadioButtonGroup rbtnMaxFPS;
        private Sunny.UI.UISwitch switchDarkMode;
        private Sunny.UI.UIRadioButtonGroup rbtnMbps;
        private Sunny.UI.UILabel uiLabel1;
        private Sunny.UI.UILinkLabel uiLinkLabel1;
        private Sunny.UI.UIGroupBox cbxOtherSetting;
        private Sunny.UI.UICheckBox cbxCloseScreen;
        private Sunny.UI.UICheckBox cbxAllFPS;
        private Sunny.UI.UICheckBox cbxKeepAwake;
        private Sunny.UI.UICheckBox cbxUseLog;
        private Sunny.UI.UITextBox tbxPort;
        private Sunny.UI.UILabel uiLabel2;
        private Sunny.UI.UICheckBox cbxHideBorder;
        private Sunny.UI.UICheckBox cbxFullScreen;
        private Sunny.UI.UICheckBox cbxTopMost;
    }
}

