using System;
using System.Runtime.InteropServices;

namespace FreeControl.Utils
{
    public static class Extend
    {
        /// <summary>
        /// 指示指定的字符串是 null、空还是仅由空白字符组成。
        /// </summary>
        /// <param name="str"></param>
        /// <returns></returns>
        public static bool IsNullOrWhiteSpace(this string str)
        {
            if (string.IsNullOrWhiteSpace(str))
            {
                return true;
            }
            else
            {
                return false;
            }
        }

        #region 拖动窗口
        [DllImport("user32.dll")]// 拖动无窗体的控件
        public static extern bool ReleaseCapture();
        [DllImport("user32.dll")]
        public static extern bool SendMessage(IntPtr hwnd, int wMsg, int wParam, int lParam);
        public const int WM_SYSCOMMAND = 0x0112;
        public const int SC_MOVE = 0xF010;
        public const int HTCAPTION = 0x0002;

        /// <summary>
        /// 拖动窗体
        /// </summary>
        public static void DragWindow(IntPtr handle)
        {
            ReleaseCapture();
            SendMessage(handle, WM_SYSCOMMAND, SC_MOVE + HTCAPTION, 0);
        }
        #endregion
    }
}
