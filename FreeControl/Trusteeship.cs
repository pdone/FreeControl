using Sunny.UI;
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Data;
using System.Drawing;
using System.Linq;
using System.Text;
using System.Windows.Forms;

namespace FreeControl
{
    public partial class Trusteeship : Form
    {
        /// <summary>
        /// 启动
        /// </summary>
        public Trusteeship()
        {
            InitializeComponent();
            UIForm main = new Main();
            //UIForm main = new Controller();
            main.Show();
        }
    }
}
