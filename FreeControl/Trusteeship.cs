using System.Windows.Forms;
using FreeControl.Utils;

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
            Singleton<Main>.Instance?.Show();
        }
    }
}
