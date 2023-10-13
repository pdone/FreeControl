using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;
using System.Timers;

namespace FreeControl.Utils
{
    public class MoveListener
    {
        #region WindowsAPi
        public delegate void WinEventDelegate(IntPtr hWinEventHook, uint eventType, IntPtr hwnd, int idObject, int idChild, uint dwEventThread, uint dwmsEventTime);

        private const uint EVENT_OBJECT_LOCATIONCHANGE = 0x800B;
        private const uint WINEVENT_OUTOFCONTEXT = 0;
        private static IntPtr hWinEventHook = IntPtr.Zero;

        [DllImport("user32.dll")]
        private static extern bool GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);

        [DllImport("user32.dll")]
        private static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);

        [DllImport("user32.dll")]
        private static extern IntPtr SetWinEventHook(uint eventMin, uint eventMax, IntPtr hmodWinEventProc, WinEventDelegate lpfnWinEventProc, uint idProcess, uint idThread, uint dwFlags);

        [DllImport("user32.dll")]
        private static extern bool UnhookWinEvent(IntPtr hWinEventHook);

        [DllImport("user32.dll")]
        private static extern IntPtr FindWindow(string lpClassName, string lpWindowName);

        [DllImport("user32.dll")]
        private static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);

        public static string GetWindowTitle(IntPtr hWnd)
        {
            const int nChars = 256;
            StringBuilder sb = new StringBuilder(nChars);
            GetWindowText(hWnd, sb, nChars);
            return sb.ToString();
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct RECT
        {
            public int Left;
            public int Top;
            public int Right;
            public int Bottom;
        }
        #endregion

        #region 钩子方式
        /// <summary>
        /// 被监控的进程ID
        /// </summary>
        private static uint idProcess = 0;

        private static void WinEventCallback(IntPtr hWinEventHook, uint eventType, IntPtr hwnd, int idObject, int idChild, uint dwEventThread, uint dwmsEventTime)
        {
            if (eventType == EVENT_OBJECT_LOCATIONCHANGE)
            {
                GetWindowThreadProcessId(hwnd, out uint processId);
                if (processId == idProcess)
                {
                    UpdateLocation(hwnd);
                }
            }
        }

        private static RECT lastRect;

        private static void UpdateLocation(IntPtr hwnd)
        {
            GetWindowRect(hwnd, out RECT rect);
            if (lastRect.Left == rect.Left && lastRect.Top == rect.Top)
                return;
            if (rect.Left + rect.Top > 0)
            {
                Main._Setting.ScrcpyPointX = rect.Left + 8;
                Main._Setting.ScrcpyPointY = rect.Top + 31;

                Main._Controller?.UpdateLocation();

                lastRect = rect;
            }
        }
        #endregion

        private static Timer timer;

        public static void StartListening(Process process, bool isHook = true)
        {
            if (isHook)// 使用钩子监控scrcpy窗口移动
            {
                idProcess = (uint)process.Id;
                hWinEventHook = SetWinEventHook(EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE, IntPtr.Zero, WinEventCallback, MoveListener.idProcess, 0, WINEVENT_OUTOFCONTEXT);
            }
            else// 无线时 使用钩子无法监控到scrcpy窗口移动 暂时使用主动查询方式实现控制器吸附
            {
                // 根据名称获取句柄
                IntPtr hwnd = FindWindow(null, Main.Info.ScrcpyTitle);
                // 最多重试20次
                int retryTimes = 20;
                while (hwnd == IntPtr.Zero)
                {
                    if (retryTimes == 0) break;
                    hwnd = FindWindow(null, Main.Info.ScrcpyTitle);
                    if (hwnd != IntPtr.Zero) break;
                    System.Threading.Thread.Sleep(500);
                    retryTimes--;
                }
                if (hwnd != IntPtr.Zero)
                {
                    timer = new Timer(15);
                    timer.Elapsed += (sender, e) =>
                    {
                        UpdateLocation(hwnd);
                    };
                    timer.Start();
                }
            }
        }

        public static void StopListening()
        {
            if (hWinEventHook != IntPtr.Zero)
                UnhookWinEvent(hWinEventHook);

            if (timer != null)
            {
                timer.Stop();
                timer.Dispose();
            }
        }
    }
}
