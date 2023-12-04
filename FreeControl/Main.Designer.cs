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
            this.linkIME = new Sunny.UI.UILinkLabel();
            this.uiLabel7 = new Sunny.UI.UILabel();
            this.tbxIp = new Sunny.UI.UITextBox();
            this.linkIssues = new Sunny.UI.UILinkLabel();
            this.tbxPort = new Sunny.UI.UITextBox();
            this.cbxUseWireless = new Sunny.UI.UICheckBox();
            this.linkSetPort = new Sunny.UI.UILinkLabel();
            this.linkEnabledADB = new Sunny.UI.UILinkLabel();
            this.uiLabel1 = new Sunny.UI.UILabel();
            this.btnStart = new Sunny.UI.UISymbolButton();
            this.comboIp = new Sunny.UI.UIComboBox();
            this.tabSetting = new System.Windows.Forms.TabPage();
            this.linkLang = new Sunny.UI.UILinkLabel();
            this.uiLabel6 = new Sunny.UI.UILabel();
            this.comboPx = new Sunny.UI.UIComboBox();
            this.uiLabel5 = new Sunny.UI.UILabel();
            this.comboMbps = new Sunny.UI.UIComboBox();
            this.uiLabel2 = new Sunny.UI.UILabel();
            this.comboMaxFPS = new Sunny.UI.UIComboBox();
            this.rbtnShortcuts = new Sunny.UI.UIRadioButtonGroup();
            this.lbAllShortcut = new Sunny.UI.UILabel();
            this.uiGroupBox1 = new Sunny.UI.UIGroupBox();
            this.uiLabel4 = new Sunny.UI.UILabel();
            this.uiLabel3 = new Sunny.UI.UILabel();
            this.updownWidth = new Sunny.UI.UIIntegerUpDown();
            this.updownHeight = new Sunny.UI.UIIntegerUpDown();
            this.cbxOtherSetting = new Sunny.UI.UIGroupBox();
            this.cbxAudioEnabled = new Sunny.UI.UICheckBox();
            this.cbxControllerEnabled = new Sunny.UI.UICheckBox();
            this.cbxShowTouches = new Sunny.UI.UICheckBox();
            this.cbxTopMost = new Sunny.UI.UICheckBox();
            this.cbxReadOnly = new Sunny.UI.UICheckBox();
            this.cbxFullScreen = new Sunny.UI.UICheckBox();
            this.cbxHideBorder = new Sunny.UI.UICheckBox();
            this.cbxUseLog = new Sunny.UI.UICheckBox();
            this.cbxKeepAwake = new Sunny.UI.UICheckBox();
            this.cbxCloseScreen = new Sunny.UI.UICheckBox();
            this.lbDarkMode = new Sunny.UI.UILabel();
            this.switchDarkMode = new Sunny.UI.UISwitch();
            this.btnClose = new Sunny.UI.UIButton();
            this.btnMini = new Sunny.UI.UIButton();
            this.navTab.SuspendLayout();
            this.tabHome.SuspendLayout();
            this.tabSetting.SuspendLayout();
            this.rbtnShortcuts.SuspendLayout();
            this.uiGroupBox1.SuspendLayout();
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
            resources.ApplyResources(this.ledTitle, "ledTitle");
            this.ledTitle.Name = "ledTitle";
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
            resources.ApplyResources(this.navTab, "navTab");
            this.navTab.Controls.Add(this.tabHome);
            this.navTab.Controls.Add(this.tabSetting);
            this.navTab.DrawMode = System.Windows.Forms.TabDrawMode.OwnerDrawFixed;
            this.navTab.FillColor = System.Drawing.Color.FromArgb(((int)(((byte)(242)))), ((int)(((byte)(242)))), ((int)(((byte)(244)))));
            this.navTab.ImageList = this.navImgList;
            this.navTab.MenuStyle = Sunny.UI.UIMenuStyle.Custom;
            this.navTab.Multiline = true;
            this.navTab.Name = "navTab";
            this.navTab.SelectedIndex = 0;
            this.navTab.SizeMode = System.Windows.Forms.TabSizeMode.Fixed;
            this.navTab.Style = Sunny.UI.UIStyle.Custom;
            this.navTab.TabBackColor = System.Drawing.Color.FromArgb(((int)(((byte)(250)))), ((int)(((byte)(250)))), ((int)(((byte)(250)))));
            this.navTab.TabSelectedColor = System.Drawing.Color.FromArgb(((int)(((byte)(240)))), ((int)(((byte)(240)))), ((int)(((byte)(240)))));
            this.navTab.TabUnSelectedForeColor = System.Drawing.Color.FromArgb(((int)(((byte)(48)))), ((int)(((byte)(48)))), ((int)(((byte)(48)))));
            // 
            // tabHome
            // 
            this.tabHome.BackColor = System.Drawing.Color.FromArgb(((int)(((byte)(242)))), ((int)(((byte)(242)))), ((int)(((byte)(244)))));
            this.tabHome.Controls.Add(this.linkIME);
            this.tabHome.Controls.Add(this.uiLabel7);
            this.tabHome.Controls.Add(this.tbxIp);
            this.tabHome.Controls.Add(this.linkIssues);
            this.tabHome.Controls.Add(this.tbxPort);
            this.tabHome.Controls.Add(this.cbxUseWireless);
            this.tabHome.Controls.Add(this.linkSetPort);
            this.tabHome.Controls.Add(this.linkEnabledADB);
            this.tabHome.Controls.Add(this.uiLabel1);
            this.tabHome.Controls.Add(this.btnStart);
            this.tabHome.Controls.Add(this.comboIp);
            resources.ApplyResources(this.tabHome, "tabHome");
            this.tabHome.Name = "tabHome";
            // 
            // linkIME
            // 
            this.linkIME.ActiveLinkColor = System.Drawing.Color.FromArgb(((int)(((byte)(220)))), ((int)(((byte)(155)))), ((int)(((byte)(40)))));
            resources.ApplyResources(this.linkIME, "linkIME");
            this.linkIME.ForeColor = System.Drawing.Color.FromArgb(((int)(((byte)(48)))), ((int)(((byte)(48)))), ((int)(((byte)(48)))));
            this.linkIME.LinkBehavior = System.Windows.Forms.LinkBehavior.HoverUnderline;
            this.linkIME.LinkColor = System.Drawing.Color.FromArgb(((int)(((byte)(48)))), ((int)(((byte)(48)))), ((int)(((byte)(48)))));
            this.linkIME.Name = "linkIME";
            this.linkIME.Style = Sunny.UI.UIStyle.Custom;
            this.linkIME.TabStop = true;
            this.linkIME.UseCompatibleTextRendering = true;
            this.linkIME.VisitedLinkColor = System.Drawing.Color.FromArgb(((int)(((byte)(230)))), ((int)(((byte)(80)))), ((int)(((byte)(80)))));
            this.linkIME.Click += new System.EventHandler(this.linkIME_Click);
            // 
            // uiLabel7
            // 
            resources.ApplyResources(this.uiLabel7, "uiLabel7");
            this.uiLabel7.ForeColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.uiLabel7.Name = "uiLabel7";
            this.uiLabel7.Style = Sunny.UI.UIStyle.Custom;
            this.uiLabel7.StyleCustomMode = true;
            // 
            // tbxIp
            // 
            this.tbxIp.Cursor = System.Windows.Forms.Cursors.IBeam;
            this.tbxIp.FillColor = System.Drawing.Color.White;
            resources.ApplyResources(this.tbxIp, "tbxIp");
            this.tbxIp.Maximum = 2147483647D;
            this.tbxIp.Minimum = -2147483648D;
            this.tbxIp.Name = "tbxIp";
            this.tbxIp.RadiusSides = ((Sunny.UI.UICornerRadiusSides)((Sunny.UI.UICornerRadiusSides.LeftTop | Sunny.UI.UICornerRadiusSides.LeftBottom)));
            this.tbxIp.RectColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.tbxIp.RectDisableColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.tbxIp.Style = Sunny.UI.UIStyle.Custom;
            this.tbxIp.StyleCustomMode = true;
            this.tbxIp.TextAlignment = System.Drawing.ContentAlignment.MiddleLeft;
            this.tbxIp.Watermark = "IP 地址";
            // 
            // linkIssues
            // 
            this.linkIssues.ActiveLinkColor = System.Drawing.Color.FromArgb(((int)(((byte)(220)))), ((int)(((byte)(155)))), ((int)(((byte)(40)))));
            resources.ApplyResources(this.linkIssues, "linkIssues");
            this.linkIssues.ForeColor = System.Drawing.Color.FromArgb(((int)(((byte)(48)))), ((int)(((byte)(48)))), ((int)(((byte)(48)))));
            this.linkIssues.LinkBehavior = System.Windows.Forms.LinkBehavior.HoverUnderline;
            this.linkIssues.LinkColor = System.Drawing.Color.FromArgb(((int)(((byte)(48)))), ((int)(((byte)(48)))), ((int)(((byte)(48)))));
            this.linkIssues.Name = "linkIssues";
            this.linkIssues.Style = Sunny.UI.UIStyle.Custom;
            this.linkIssues.TabStop = true;
            this.linkIssues.UseCompatibleTextRendering = true;
            this.linkIssues.VisitedLinkColor = System.Drawing.Color.FromArgb(((int)(((byte)(230)))), ((int)(((byte)(80)))), ((int)(((byte)(80)))));
            this.linkIssues.Click += new System.EventHandler(this.linkIssues_Click);
            // 
            // tbxPort
            // 
            this.tbxPort.Cursor = System.Windows.Forms.Cursors.IBeam;
            this.tbxPort.FillColor = System.Drawing.Color.White;
            resources.ApplyResources(this.tbxPort, "tbxPort");
            this.tbxPort.Maximum = 2147483647D;
            this.tbxPort.Minimum = -2147483648D;
            this.tbxPort.Name = "tbxPort";
            this.tbxPort.RectColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.tbxPort.RectDisableColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.tbxPort.Style = Sunny.UI.UIStyle.Custom;
            this.tbxPort.StyleCustomMode = true;
            this.tbxPort.TextAlignment = System.Drawing.ContentAlignment.MiddleLeft;
            this.tbxPort.Watermark = "端口号";
            // 
            // cbxUseWireless
            // 
            this.cbxUseWireless.CheckBoxColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.cbxUseWireless.Cursor = System.Windows.Forms.Cursors.Hand;
            resources.ApplyResources(this.cbxUseWireless, "cbxUseWireless");
            this.cbxUseWireless.Name = "cbxUseWireless";
            this.cbxUseWireless.Style = Sunny.UI.UIStyle.Custom;
            // 
            // linkSetPort
            // 
            this.linkSetPort.ActiveLinkColor = System.Drawing.Color.FromArgb(((int)(((byte)(220)))), ((int)(((byte)(155)))), ((int)(((byte)(40)))));
            resources.ApplyResources(this.linkSetPort, "linkSetPort");
            this.linkSetPort.ForeColor = System.Drawing.Color.FromArgb(((int)(((byte)(48)))), ((int)(((byte)(48)))), ((int)(((byte)(48)))));
            this.linkSetPort.LinkBehavior = System.Windows.Forms.LinkBehavior.HoverUnderline;
            this.linkSetPort.LinkColor = System.Drawing.Color.FromArgb(((int)(((byte)(48)))), ((int)(((byte)(48)))), ((int)(((byte)(48)))));
            this.linkSetPort.Name = "linkSetPort";
            this.linkSetPort.Style = Sunny.UI.UIStyle.Custom;
            this.linkSetPort.TabStop = true;
            this.linkSetPort.UseCompatibleTextRendering = true;
            this.linkSetPort.VisitedLinkColor = System.Drawing.Color.FromArgb(((int)(((byte)(230)))), ((int)(((byte)(80)))), ((int)(((byte)(80)))));
            this.linkSetPort.Click += new System.EventHandler(this.linkSetPort_Click);
            // 
            // linkEnabledADB
            // 
            this.linkEnabledADB.ActiveLinkColor = System.Drawing.Color.FromArgb(((int)(((byte)(220)))), ((int)(((byte)(155)))), ((int)(((byte)(40)))));
            resources.ApplyResources(this.linkEnabledADB, "linkEnabledADB");
            this.linkEnabledADB.ForeColor = System.Drawing.Color.FromArgb(((int)(((byte)(48)))), ((int)(((byte)(48)))), ((int)(((byte)(48)))));
            this.linkEnabledADB.LinkBehavior = System.Windows.Forms.LinkBehavior.HoverUnderline;
            this.linkEnabledADB.LinkColor = System.Drawing.Color.FromArgb(((int)(((byte)(48)))), ((int)(((byte)(48)))), ((int)(((byte)(48)))));
            this.linkEnabledADB.Name = "linkEnabledADB";
            this.linkEnabledADB.Style = Sunny.UI.UIStyle.Custom;
            this.linkEnabledADB.TabStop = true;
            this.linkEnabledADB.VisitedLinkColor = System.Drawing.Color.FromArgb(((int)(((byte)(230)))), ((int)(((byte)(80)))), ((int)(((byte)(80)))));
            this.linkEnabledADB.Click += new System.EventHandler(this.linkEnabledADB_Click);
            // 
            // uiLabel1
            // 
            resources.ApplyResources(this.uiLabel1, "uiLabel1");
            this.uiLabel1.ForeColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.uiLabel1.Name = "uiLabel1";
            this.uiLabel1.Style = Sunny.UI.UIStyle.Custom;
            this.uiLabel1.StyleCustomMode = true;
            // 
            // btnStart
            // 
            this.btnStart.Cursor = System.Windows.Forms.Cursors.Hand;
            this.btnStart.FillColor = System.Drawing.Color.FromArgb(((int)(((byte)(15)))), ((int)(((byte)(40)))), ((int)(((byte)(70)))));
            this.btnStart.FillHoverColor = System.Drawing.Color.FromArgb(((int)(((byte)(216)))), ((int)(((byte)(233)))), ((int)(((byte)(255)))));
            this.btnStart.FillPressColor = System.Drawing.Color.FromArgb(((int)(((byte)(235)))), ((int)(((byte)(243)))), ((int)(((byte)(255)))));
            resources.ApplyResources(this.btnStart, "btnStart");
            this.btnStart.ForeHoverColor = System.Drawing.Color.FromArgb(((int)(((byte)(130)))), ((int)(((byte)(130)))), ((int)(((byte)(130)))));
            this.btnStart.ForePressColor = System.Drawing.Color.FromArgb(((int)(((byte)(130)))), ((int)(((byte)(130)))), ((int)(((byte)(130)))));
            this.btnStart.Name = "btnStart";
            this.btnStart.RectColor = System.Drawing.Color.FromArgb(((int)(((byte)(130)))), ((int)(((byte)(130)))), ((int)(((byte)(130)))));
            this.btnStart.RectHoverColor = System.Drawing.Color.FromArgb(((int)(((byte)(130)))), ((int)(((byte)(130)))), ((int)(((byte)(130)))));
            this.btnStart.RectPressColor = System.Drawing.Color.FromArgb(((int)(((byte)(130)))), ((int)(((byte)(130)))), ((int)(((byte)(130)))));
            this.btnStart.Style = Sunny.UI.UIStyle.Custom;
            this.btnStart.Symbol = 61671;
            // 
            // comboIp
            // 
            this.comboIp.DropDownStyle = Sunny.UI.UIDropDownStyle.DropDownList;
            this.comboIp.FillColor = System.Drawing.Color.White;
            resources.ApplyResources(this.comboIp, "comboIp");
            this.comboIp.Name = "comboIp";
            this.comboIp.RectColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.comboIp.Style = Sunny.UI.UIStyle.Custom;
            this.comboIp.TextAlignment = System.Drawing.ContentAlignment.MiddleLeft;
            this.comboIp.Watermark = "";
            // 
            // tabSetting
            // 
            this.tabSetting.BackColor = System.Drawing.Color.FromArgb(((int)(((byte)(242)))), ((int)(((byte)(242)))), ((int)(((byte)(244)))));
            this.tabSetting.Controls.Add(this.linkLang);
            this.tabSetting.Controls.Add(this.uiLabel6);
            this.tabSetting.Controls.Add(this.comboPx);
            this.tabSetting.Controls.Add(this.uiLabel5);
            this.tabSetting.Controls.Add(this.comboMbps);
            this.tabSetting.Controls.Add(this.uiLabel2);
            this.tabSetting.Controls.Add(this.comboMaxFPS);
            this.tabSetting.Controls.Add(this.rbtnShortcuts);
            this.tabSetting.Controls.Add(this.uiGroupBox1);
            this.tabSetting.Controls.Add(this.cbxOtherSetting);
            this.tabSetting.Controls.Add(this.lbDarkMode);
            this.tabSetting.Controls.Add(this.switchDarkMode);
            resources.ApplyResources(this.tabSetting, "tabSetting");
            this.tabSetting.Name = "tabSetting";
            // 
            // linkLang
            // 
            this.linkLang.ActiveLinkColor = System.Drawing.Color.FromArgb(((int)(((byte)(220)))), ((int)(((byte)(155)))), ((int)(((byte)(40)))));
            resources.ApplyResources(this.linkLang, "linkLang");
            this.linkLang.ForeColor = System.Drawing.Color.FromArgb(((int)(((byte)(48)))), ((int)(((byte)(48)))), ((int)(((byte)(48)))));
            this.linkLang.LinkBehavior = System.Windows.Forms.LinkBehavior.HoverUnderline;
            this.linkLang.LinkColor = System.Drawing.Color.FromArgb(((int)(((byte)(48)))), ((int)(((byte)(48)))), ((int)(((byte)(48)))));
            this.linkLang.Name = "linkLang";
            this.linkLang.Style = Sunny.UI.UIStyle.Custom;
            this.linkLang.TabStop = true;
            this.linkLang.UseCompatibleTextRendering = true;
            this.linkLang.VisitedLinkColor = System.Drawing.Color.FromArgb(((int)(((byte)(230)))), ((int)(((byte)(80)))), ((int)(((byte)(80)))));
            this.linkLang.Click += new System.EventHandler(this.linkLang_Click);
            // 
            // uiLabel6
            // 
            resources.ApplyResources(this.uiLabel6, "uiLabel6");
            this.uiLabel6.Name = "uiLabel6";
            this.uiLabel6.Style = Sunny.UI.UIStyle.Custom;
            // 
            // comboPx
            // 
            this.comboPx.DropDownStyle = Sunny.UI.UIDropDownStyle.DropDownList;
            this.comboPx.FillColor = System.Drawing.Color.White;
            resources.ApplyResources(this.comboPx, "comboPx");
            this.comboPx.Items.AddRange(new object[] {
            resources.GetString("comboPx.Items"),
            resources.GetString("comboPx.Items1"),
            resources.GetString("comboPx.Items2"),
            resources.GetString("comboPx.Items3"),
            resources.GetString("comboPx.Items4"),
            resources.GetString("comboPx.Items5")});
            this.comboPx.Name = "comboPx";
            this.comboPx.Style = Sunny.UI.UIStyle.Custom;
            this.comboPx.TextAlignment = System.Drawing.ContentAlignment.MiddleLeft;
            // 
            // uiLabel5
            // 
            resources.ApplyResources(this.uiLabel5, "uiLabel5");
            this.uiLabel5.Name = "uiLabel5";
            this.uiLabel5.Style = Sunny.UI.UIStyle.Custom;
            // 
            // comboMbps
            // 
            this.comboMbps.DropDownStyle = Sunny.UI.UIDropDownStyle.DropDownList;
            this.comboMbps.FillColor = System.Drawing.Color.White;
            resources.ApplyResources(this.comboMbps, "comboMbps");
            this.comboMbps.Items.AddRange(new object[] {
            resources.GetString("comboMbps.Items"),
            resources.GetString("comboMbps.Items1"),
            resources.GetString("comboMbps.Items2"),
            resources.GetString("comboMbps.Items3"),
            resources.GetString("comboMbps.Items4"),
            resources.GetString("comboMbps.Items5")});
            this.comboMbps.Name = "comboMbps";
            this.comboMbps.Style = Sunny.UI.UIStyle.Custom;
            this.comboMbps.TextAlignment = System.Drawing.ContentAlignment.MiddleLeft;
            // 
            // uiLabel2
            // 
            resources.ApplyResources(this.uiLabel2, "uiLabel2");
            this.uiLabel2.Name = "uiLabel2";
            this.uiLabel2.Style = Sunny.UI.UIStyle.Custom;
            // 
            // comboMaxFPS
            // 
            this.comboMaxFPS.DropDownStyle = Sunny.UI.UIDropDownStyle.DropDownList;
            this.comboMaxFPS.FillColor = System.Drawing.Color.White;
            resources.ApplyResources(this.comboMaxFPS, "comboMaxFPS");
            this.comboMaxFPS.Items.AddRange(new object[] {
            resources.GetString("comboMaxFPS.Items"),
            resources.GetString("comboMaxFPS.Items1"),
            resources.GetString("comboMaxFPS.Items2"),
            resources.GetString("comboMaxFPS.Items3"),
            resources.GetString("comboMaxFPS.Items4"),
            resources.GetString("comboMaxFPS.Items5")});
            this.comboMaxFPS.Name = "comboMaxFPS";
            this.comboMaxFPS.Style = Sunny.UI.UIStyle.Custom;
            this.comboMaxFPS.TextAlignment = System.Drawing.ContentAlignment.MiddleLeft;
            // 
            // rbtnShortcuts
            // 
            this.rbtnShortcuts.BackColor = System.Drawing.Color.Transparent;
            this.rbtnShortcuts.Controls.Add(this.lbAllShortcut);
            this.rbtnShortcuts.FillColor = System.Drawing.Color.FromArgb(((int)(((byte)(242)))), ((int)(((byte)(242)))), ((int)(((byte)(244)))));
            resources.ApplyResources(this.rbtnShortcuts, "rbtnShortcuts");
            this.rbtnShortcuts.Items.AddRange(new object[] {
            resources.GetString("rbtnShortcuts.Items"),
            resources.GetString("rbtnShortcuts.Items1"),
            resources.GetString("rbtnShortcuts.Items2")});
            this.rbtnShortcuts.ItemSize = new System.Drawing.Size(100, 27);
            this.rbtnShortcuts.Name = "rbtnShortcuts";
            this.rbtnShortcuts.Radius = 10;
            this.rbtnShortcuts.RectColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.rbtnShortcuts.Style = Sunny.UI.UIStyle.Custom;
            this.rbtnShortcuts.TextAlignment = System.Drawing.ContentAlignment.MiddleCenter;
            // 
            // lbAllShortcut
            // 
            resources.ApplyResources(this.lbAllShortcut, "lbAllShortcut");
            this.lbAllShortcut.BackColor = System.Drawing.Color.Transparent;
            this.lbAllShortcut.Cursor = System.Windows.Forms.Cursors.Hand;
            this.lbAllShortcut.Name = "lbAllShortcut";
            this.lbAllShortcut.Style = Sunny.UI.UIStyle.Custom;
            this.lbAllShortcut.Click += new System.EventHandler(this.lbAllShortcut_Click);
            // 
            // uiGroupBox1
            // 
            this.uiGroupBox1.Controls.Add(this.uiLabel4);
            this.uiGroupBox1.Controls.Add(this.uiLabel3);
            this.uiGroupBox1.Controls.Add(this.updownWidth);
            this.uiGroupBox1.Controls.Add(this.updownHeight);
            this.uiGroupBox1.FillColor = System.Drawing.Color.FromArgb(((int)(((byte)(242)))), ((int)(((byte)(242)))), ((int)(((byte)(244)))));
            resources.ApplyResources(this.uiGroupBox1, "uiGroupBox1");
            this.uiGroupBox1.Name = "uiGroupBox1";
            this.uiGroupBox1.RectColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.uiGroupBox1.Style = Sunny.UI.UIStyle.Custom;
            this.uiGroupBox1.TextAlignment = System.Drawing.ContentAlignment.MiddleCenter;
            // 
            // uiLabel4
            // 
            resources.ApplyResources(this.uiLabel4, "uiLabel4");
            this.uiLabel4.Name = "uiLabel4";
            this.uiLabel4.Style = Sunny.UI.UIStyle.Custom;
            // 
            // uiLabel3
            // 
            resources.ApplyResources(this.uiLabel3, "uiLabel3");
            this.uiLabel3.Name = "uiLabel3";
            this.uiLabel3.Style = Sunny.UI.UIStyle.Custom;
            // 
            // updownWidth
            // 
            resources.ApplyResources(this.updownWidth, "updownWidth");
            this.updownWidth.HasMaximum = true;
            this.updownWidth.HasMinimum = true;
            this.updownWidth.Maximum = 5555;
            this.updownWidth.Minimum = 0;
            this.updownWidth.Name = "updownWidth";
            this.updownWidth.Style = Sunny.UI.UIStyle.Custom;
            this.updownWidth.TextAlignment = System.Drawing.ContentAlignment.MiddleCenter;
            // 
            // updownHeight
            // 
            resources.ApplyResources(this.updownHeight, "updownHeight");
            this.updownHeight.HasMaximum = true;
            this.updownHeight.HasMinimum = true;
            this.updownHeight.Maximum = 5555;
            this.updownHeight.Minimum = 0;
            this.updownHeight.Name = "updownHeight";
            this.updownHeight.Style = Sunny.UI.UIStyle.Custom;
            this.updownHeight.TextAlignment = System.Drawing.ContentAlignment.MiddleCenter;
            // 
            // cbxOtherSetting
            // 
            this.cbxOtherSetting.Controls.Add(this.cbxAudioEnabled);
            this.cbxOtherSetting.Controls.Add(this.cbxControllerEnabled);
            this.cbxOtherSetting.Controls.Add(this.cbxShowTouches);
            this.cbxOtherSetting.Controls.Add(this.cbxTopMost);
            this.cbxOtherSetting.Controls.Add(this.cbxReadOnly);
            this.cbxOtherSetting.Controls.Add(this.cbxFullScreen);
            this.cbxOtherSetting.Controls.Add(this.cbxHideBorder);
            this.cbxOtherSetting.Controls.Add(this.cbxUseLog);
            this.cbxOtherSetting.Controls.Add(this.cbxKeepAwake);
            this.cbxOtherSetting.Controls.Add(this.cbxCloseScreen);
            this.cbxOtherSetting.FillColor = System.Drawing.Color.FromArgb(((int)(((byte)(242)))), ((int)(((byte)(242)))), ((int)(((byte)(244)))));
            resources.ApplyResources(this.cbxOtherSetting, "cbxOtherSetting");
            this.cbxOtherSetting.Name = "cbxOtherSetting";
            this.cbxOtherSetting.RectColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.cbxOtherSetting.Style = Sunny.UI.UIStyle.Custom;
            this.cbxOtherSetting.TextAlignment = System.Drawing.ContentAlignment.MiddleCenter;
            // 
            // cbxAudioEnabled
            // 
            this.cbxAudioEnabled.Checked = true;
            this.cbxAudioEnabled.Cursor = System.Windows.Forms.Cursors.Hand;
            resources.ApplyResources(this.cbxAudioEnabled, "cbxAudioEnabled");
            this.cbxAudioEnabled.Name = "cbxAudioEnabled";
            this.cbxAudioEnabled.Style = Sunny.UI.UIStyle.Custom;
            // 
            // cbxControllerEnabled
            // 
            this.cbxControllerEnabled.Cursor = System.Windows.Forms.Cursors.Hand;
            resources.ApplyResources(this.cbxControllerEnabled, "cbxControllerEnabled");
            this.cbxControllerEnabled.Name = "cbxControllerEnabled";
            this.cbxControllerEnabled.Style = Sunny.UI.UIStyle.Custom;
            // 
            // cbxShowTouches
            // 
            this.cbxShowTouches.Cursor = System.Windows.Forms.Cursors.Hand;
            resources.ApplyResources(this.cbxShowTouches, "cbxShowTouches");
            this.cbxShowTouches.Name = "cbxShowTouches";
            this.cbxShowTouches.Style = Sunny.UI.UIStyle.Custom;
            // 
            // cbxTopMost
            // 
            this.cbxTopMost.Cursor = System.Windows.Forms.Cursors.Hand;
            resources.ApplyResources(this.cbxTopMost, "cbxTopMost");
            this.cbxTopMost.Name = "cbxTopMost";
            this.cbxTopMost.Style = Sunny.UI.UIStyle.Custom;
            // 
            // cbxReadOnly
            // 
            this.cbxReadOnly.Cursor = System.Windows.Forms.Cursors.Hand;
            resources.ApplyResources(this.cbxReadOnly, "cbxReadOnly");
            this.cbxReadOnly.Name = "cbxReadOnly";
            this.cbxReadOnly.Style = Sunny.UI.UIStyle.Custom;
            // 
            // cbxFullScreen
            // 
            this.cbxFullScreen.Cursor = System.Windows.Forms.Cursors.Hand;
            resources.ApplyResources(this.cbxFullScreen, "cbxFullScreen");
            this.cbxFullScreen.Name = "cbxFullScreen";
            this.cbxFullScreen.Style = Sunny.UI.UIStyle.Custom;
            // 
            // cbxHideBorder
            // 
            this.cbxHideBorder.Cursor = System.Windows.Forms.Cursors.Hand;
            resources.ApplyResources(this.cbxHideBorder, "cbxHideBorder");
            this.cbxHideBorder.Name = "cbxHideBorder";
            this.cbxHideBorder.Style = Sunny.UI.UIStyle.Custom;
            // 
            // cbxUseLog
            // 
            this.cbxUseLog.Cursor = System.Windows.Forms.Cursors.Hand;
            resources.ApplyResources(this.cbxUseLog, "cbxUseLog");
            this.cbxUseLog.Name = "cbxUseLog";
            this.cbxUseLog.Style = Sunny.UI.UIStyle.Custom;
            // 
            // cbxKeepAwake
            // 
            this.cbxKeepAwake.Cursor = System.Windows.Forms.Cursors.Hand;
            resources.ApplyResources(this.cbxKeepAwake, "cbxKeepAwake");
            this.cbxKeepAwake.Name = "cbxKeepAwake";
            this.cbxKeepAwake.Style = Sunny.UI.UIStyle.Custom;
            // 
            // cbxCloseScreen
            // 
            this.cbxCloseScreen.Cursor = System.Windows.Forms.Cursors.Hand;
            resources.ApplyResources(this.cbxCloseScreen, "cbxCloseScreen");
            this.cbxCloseScreen.Name = "cbxCloseScreen";
            this.cbxCloseScreen.Style = Sunny.UI.UIStyle.Custom;
            // 
            // lbDarkMode
            // 
            resources.ApplyResources(this.lbDarkMode, "lbDarkMode");
            this.lbDarkMode.Name = "lbDarkMode";
            this.lbDarkMode.Style = Sunny.UI.UIStyle.Custom;
            // 
            // switchDarkMode
            // 
            this.switchDarkMode.ActiveText = "ON";
            this.switchDarkMode.Cursor = System.Windows.Forms.Cursors.Hand;
            resources.ApplyResources(this.switchDarkMode, "switchDarkMode");
            this.switchDarkMode.InActiveText = "OFF";
            this.switchDarkMode.Name = "switchDarkMode";
            this.switchDarkMode.Style = Sunny.UI.UIStyle.Custom;
            // 
            // btnClose
            // 
            this.btnClose.Cursor = System.Windows.Forms.Cursors.Hand;
            resources.ApplyResources(this.btnClose, "btnClose");
            this.btnClose.Name = "btnClose";
            this.btnClose.Style = Sunny.UI.UIStyle.Custom;
            // 
            // btnMini
            // 
            this.btnMini.Cursor = System.Windows.Forms.Cursors.Hand;
            resources.ApplyResources(this.btnMini, "btnMini");
            this.btnMini.Name = "btnMini";
            this.btnMini.Style = Sunny.UI.UIStyle.Custom;
            // 
            // Main
            // 
            this.AllowShowTitle = false;
            this.AutoScaleMode = System.Windows.Forms.AutoScaleMode.None;
            this.BackColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            resources.ApplyResources(this, "$this");
            this.Controls.Add(this.btnMini);
            this.Controls.Add(this.ledTitle);
            this.Controls.Add(this.btnClose);
            this.Controls.Add(this.navTab);
            this.MaximizeBox = false;
            this.Name = "Main";
            this.RectColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.ShowRect = false;
            this.ShowTitle = false;
            this.ShowTitleIcon = true;
            this.Style = Sunny.UI.UIStyle.Custom;
            this.TitleColor = System.Drawing.Color.FromArgb(((int)(((byte)(140)))), ((int)(((byte)(140)))), ((int)(((byte)(140)))));
            this.TitleHeight = 63;
            this.navTab.ResumeLayout(false);
            this.tabHome.ResumeLayout(false);
            this.tabHome.PerformLayout();
            this.tabSetting.ResumeLayout(false);
            this.tabSetting.PerformLayout();
            this.rbtnShortcuts.ResumeLayout(false);
            this.rbtnShortcuts.PerformLayout();
            this.uiGroupBox1.ResumeLayout(false);
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
        private Sunny.UI.UILabel lbDarkMode;
        private Sunny.UI.UISwitch switchDarkMode;
        private Sunny.UI.UILabel uiLabel1;
        private Sunny.UI.UILinkLabel linkEnabledADB;
        private Sunny.UI.UIGroupBox cbxOtherSetting;
        private Sunny.UI.UICheckBox cbxCloseScreen;
        private Sunny.UI.UICheckBox cbxKeepAwake;
        private Sunny.UI.UICheckBox cbxUseLog;
        private Sunny.UI.UICheckBox cbxHideBorder;
        private Sunny.UI.UICheckBox cbxFullScreen;
        private Sunny.UI.UICheckBox cbxTopMost;
        private Sunny.UI.UILinkLabel linkSetPort;
        private Sunny.UI.UITextBox tbxPort;
        private Sunny.UI.UICheckBox cbxUseWireless;
        private Sunny.UI.UITextBox tbxIp;
        private Sunny.UI.UIGroupBox uiGroupBox1;
        private Sunny.UI.UILabel uiLabel4;
        private Sunny.UI.UILabel uiLabel3;
        private Sunny.UI.UIIntegerUpDown updownWidth;
        private Sunny.UI.UIIntegerUpDown updownHeight;
        private Sunny.UI.UICheckBox cbxReadOnly;
        private Sunny.UI.UICheckBox cbxShowTouches;
        private Sunny.UI.UILinkLabel linkIssues;
        private Sunny.UI.UIRadioButtonGroup rbtnShortcuts;
        private Sunny.UI.UILabel lbAllShortcut;
        private Sunny.UI.UIButton btnClose;
        private Sunny.UI.UIButton btnMini;
        private Sunny.UI.UIComboBox comboMaxFPS;
        private Sunny.UI.UILabel uiLabel2;
        private Sunny.UI.UILabel uiLabel5;
        private Sunny.UI.UIComboBox comboMbps;
        private Sunny.UI.UILabel uiLabel6;
        private Sunny.UI.UIComboBox comboPx;
        private Sunny.UI.UICheckBox cbxControllerEnabled;
        private Sunny.UI.UICheckBox cbxAudioEnabled;
        private Sunny.UI.UIComboBox comboIp;
        private Sunny.UI.UILabel uiLabel7;
        private Sunny.UI.UILinkLabel linkIME;
        private Sunny.UI.UILinkLabel linkLang;
    }
}

