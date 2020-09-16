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
            Application.Run(new Trusteeship());
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
                case "ICSharpCode.SharpZipLib":
                    assembly = Assembly.Load(Properties.Resources.ICSharpCode_SharpZipLib);
                    break;
                default:
                    break;
            }
            return assembly;
        }
    }
}
