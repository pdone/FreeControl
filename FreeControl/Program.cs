using System;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Windows.Forms;
using FreeControl;
using FreeControl.Utils;

[assembly: AssemblyTitle("在PC上控制Android设备")]//在PC上控制Android设备。
[assembly: AssemblyDescription("")]
[assembly: AssemblyConfiguration("")]
[assembly: AssemblyCompany("Pdone Technology Ltd.")]
[assembly: AssemblyProduct("Free Control")]
[assembly: AssemblyCopyright("Copyright © 2024 awaw.cc")]
[assembly: AssemblyTrademark("")]
[assembly: AssemblyCulture("")]

[assembly: ComVisible(false)]

[assembly: Guid("ef7540e1-f2e7-4682-80ab-92354c55a4c6")]

[assembly: AssemblyFileVersion(Program.Version)]
[assembly: AssemblyVersion(Program.Version)]

namespace FreeControl
{
    static class Program
    {
        public const string Version = "1.7.3";
        /// <summary>
        /// 应用程序的主入口点。
        /// </summary>
        [STAThread]
        static void Main()
        {
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            try
            {
                Application.Run(Singleton<Main>.Instance);
            }
            catch (Exception ex)
            {
                Logger.Error(ex);
                DialogResult dr = MessageBox.Show("启动失败！是否要向开发者反馈此问题？", "提示", MessageBoxButtons.OKCancel);
                if (dr == DialogResult.OK)
                {
                    System.Diagnostics.Process.Start("https://github.com/pdone/FreeControl/issues");
                }
            }
        }
    }
}
