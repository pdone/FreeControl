using FreeControl.Utils;
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Data;
using System.Drawing;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading;
using System.Windows.Forms;

namespace FreeControl
{
    public partial class Loading : Form
    {
        private string UserDataPath;
        public string ScrcpyPath = string.Empty;
        public Loading(string UserDataPath)
        {
            this.UserDataPath = UserDataPath;
            InitializeComponent();
        }

        public Loading()
        {
            InitializeComponent();
        }

        private void Loading_Load(object sender, EventArgs e)
        {
            //ScrcpyPath = Path.Combine(UserDataPath, "scrcpy_win32_v1_16/");
            //string tempFileName = "temp.zip";
            //if (!Directory.Exists(ScrcpyPath))
            //{
            //    Directory.CreateDirectory(ScrcpyPath);
            //    File.WriteAllBytes(ScrcpyPath + tempFileName, Properties.Resources.scrcpy_win32_v1_16);
            //    if (SharpZip.UnpackFiles(ScrcpyPath + tempFileName, ScrcpyPath))
            //    {
            //        File.Delete(ScrcpyPath + tempFileName);
            //        this.DialogResult = DialogResult.OK;
            //    }
            //}
            //else
            //{
            //    this.DialogResult = DialogResult.OK;
            //}
        }
    }
}
