using FreeControl.Utils;
using System;
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
