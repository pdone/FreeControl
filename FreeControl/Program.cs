using FreeControl.Utils;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Windows.Forms;

namespace FreeControl
{
    static class Program
    {
        /// <summary>
        /// 应用程序的主入口点。
        /// </summary>
        [STAThread]
        static void Main()
        {
            AppDomain.CurrentDomain.AssemblyResolve += CurrentDomain_AssemblyResolve;

            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            try
            {
                Application.Run(new Trusteeship());
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
        private static Assembly CurrentDomain_AssemblyResolve(object sender, ResolveEventArgs args)
        {
            string dllName = new AssemblyName(args.Name).Name;
            Assembly assembly = null;
            switch (dllName)
            {
                case "SunnyUI":
                    assembly = Assembly.Load(Properties.Resources.SunnyUI);
                    break;
                case "SunnyUI.Common":
                    assembly = Assembly.Load(Properties.Resources.SunnyUI_Common);
                    break;
                //case "ICSharpCode.SharpZipLib":
                //    assembly = Assembly.Load(Properties.Resources.ICSharpCode_SharpZipLib);
                //    break;
                //case "Newtonsoft.Json":
                //    assembly = Assembly.Load(Properties.Resources.Newtonsoft_Json);
                //    break;
                default:
                    break;
            }
            return assembly;
        }
    }
}
