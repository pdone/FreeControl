using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;
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

        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        static extern bool ShowWindow(IntPtr hWnd, ShowWindowCommands nCmdShow);

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

        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        static extern bool GetWindowPlacement(IntPtr hWnd, ref WINDOWPLACEMENT lpwndpl);

        // 定义窗口状态的结构体
        [Serializable]
        [StructLayout(LayoutKind.Sequential)]
        struct WINDOWPLACEMENT
        {
            public int length;
            public int flags;
            public ShowWindowCommands showCmd;
            public POINT ptMinPosition;
            public POINT ptMaxPosition;
            public RECT rcNormalPosition;
        }

        // 定义显示窗口的命令
        enum ShowWindowCommands : int
        {
            SW_HIDE = 0,                    // 隐藏窗口
            SW_SHOWNORMAL = 1,              // 正常显示窗口
            // SW_NORMAL = 1,                  // 与 SW_SHOWNORMAL 相同
            SW_SHOWMINIMIZED = 2,           // 最小化窗口
            SW_SHOWMAXIMIZED = 3,           // 最大化窗口
            // SW_MAXIMIZE = 3,                // 与 SW_SHOWMAXIMIZED 相同
            SW_SHOWNOACTIVATE = 4,          // 以激活状态显示窗口，但不激活它
            SW_SHOW = 5,                    // 以当前大小和位置显示窗口
            SW_MINIMIZE = 6,                // 最小化窗口
            SW_SHOWMINNOACTIVE = 7,         // 以最小化状态显示窗口，但不激活它
            SW_SHOWNA = 8,                  // 在当前位置以非激活状态显示窗口
            SW_RESTORE = 9,                 // 恢复窗口的大小和位置
            SW_SHOWDEFAULT = 10,            // 使用创建时的默认大小和位置显示窗口
            SW_FORCEMINIMIZE = 11           // 最小化窗口，即使程序不响应
        }

        [Serializable]
        [StructLayout(LayoutKind.Sequential)]
        struct POINT
        {
            public int x;
            public int y;
        }

        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);

        // 定义窗口位置和大小的标志
        const uint SWP_NOSIZE = 0x0001;       // 不改变窗口大小
        const uint SWP_NOMOVE = 0x0002;       // 不改变窗口位置
        const uint SWP_NOZORDER = 0x0004;     // 不改变窗口Z顺序
        const uint SWP_NOACTIVATE = 0x0010;   // 不激活窗口

        // 指定置顶窗口
        static IntPtr HWND_TOPMOST = new IntPtr(-1);
        // 指定取消置顶窗口
        static IntPtr HWND_NOTOPMOST = new IntPtr(-2);
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
                    UpdateStatus(hwnd);
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

                if (Main.ControllerPtr != IntPtr.Zero)
                {
                    // 使用winapi更新控制器位置
                    SetWindowPos(Main.ControllerPtr, IntPtr.Zero,
                        Main._Setting.ScrcpyPointX - 57, Main._Setting.ScrcpyPointY,
                        0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
                }

                lastRect = rect;
            }
        }

        // 创建WINDOWPLACEMENT结构体
        private static WINDOWPLACEMENT placement = new WINDOWPLACEMENT()
        {
            length = Marshal.SizeOf(placement)
        };

        private static void UpdateStatus(IntPtr hwnd)
        {
            if (hwnd == IntPtr.Zero || Main.ControllerPtr == IntPtr.Zero)
                return;
            if (GetWindowPlacement(hwnd, ref placement))
            {

                switch (placement.showCmd)
                {
                    case ShowWindowCommands.SW_HIDE:
                    case ShowWindowCommands.SW_SHOWMINIMIZED:
                    case ShowWindowCommands.SW_SHOWMAXIMIZED:
                    case ShowWindowCommands.SW_SHOWMINNOACTIVE:
                    case ShowWindowCommands.SW_FORCEMINIMIZE:
                        SetWindowPos(Main.ControllerPtr, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                        ShowWindow(Main.ControllerPtr, ShowWindowCommands.SW_HIDE);
                        break;
                    case ShowWindowCommands.SW_SHOWNORMAL:
                    case ShowWindowCommands.SW_SHOWNOACTIVATE:
                    case ShowWindowCommands.SW_SHOW:
                    case ShowWindowCommands.SW_SHOWNA:
                    case ShowWindowCommands.SW_RESTORE:
                    case ShowWindowCommands.SW_SHOWDEFAULT:
                        SetWindowPos(Main.ControllerPtr, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                        ShowWindow(Main.ControllerPtr, ShowWindowCommands.SW_SHOWNOACTIVATE);
                        break;
                }
            }
        }
        #endregion

        private static Timer timer;

        private static GCHandle handle;

        public static void StartListening(Process process, bool isHook = true)
        {
            if (isHook)// 使用钩子监控scrcpy窗口移动
            {
                WinEventDelegate myDelegate = WinEventCallback;
                handle = GCHandle.Alloc(myDelegate);// 手动管理内存 避免被回收

                idProcess = (uint)process.Id;
                hWinEventHook = SetWinEventHook(EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE, IntPtr.Zero, (WinEventDelegate)handle.Target, MoveListener.idProcess, 0, WINEVENT_OUTOFCONTEXT);
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
                        UpdateStatus(hwnd);
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

            if (handle.IsAllocated)
                handle.Free();
        }
    }
}
