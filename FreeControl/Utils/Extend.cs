using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Reflection;
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

        public static bool IsNotNull(this string str)
        {
            if (!string.IsNullOrWhiteSpace(str))
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

        /// <summary>
        /// 获取实体中指定成员Description特性值
        /// </summary>
        /// <typeparam name="T"></typeparam>
        /// <param name="obj"></param>
        /// <param name="name"></param>
        /// <returns></returns>
        public static string GetDesc<T>(this T obj, string name) where T : Setting
        {
            T ent = obj;
            var res = "";
            foreach (var item in ent.GetType().GetProperties())
            {
                if (item.Name != name)
                {
                    continue;
                }
                var v = (DescriptionAttribute[])item.GetCustomAttributes(typeof(DescriptionAttribute), false);
                if (v != null && v.Count() > 0)
                {
                    res = v[0].Description;
                    return res;
                }
            }
            return res;
        }

        public static string GetDesc(this Enum value)
        {
            FieldInfo field = value.GetType().GetField(value.ToString());

            if (field != null)
            {
                DescriptionAttribute attribute =
                    (DescriptionAttribute)Attribute.GetCustomAttribute(field, typeof(DescriptionAttribute));

                if (attribute != null)
                {
                    return attribute.Description;
                }
            }

            return value.ToString();
        }

        public static List<string> GetEnumDescList<T>() where T : Enum
        {
            List<string> list = new List<string>();
            T[] enumValues = (T[])Enum.GetValues(typeof(T));

            foreach (T item in enumValues)
            {
                list.Add(item.GetDesc());
            }
            return list;
        }
    }

    public abstract class Singleton<T> where T : class
    {
        private static T _Instance;
        private static readonly object SyncObject = new object();

        public static T Instance
        {
            get
            {
                if (_Instance == null)
                {
                    lock (SyncObject)
                    {
                        if (_Instance == null)
                        {
                            _Instance = (T)Activator.CreateInstance(typeof(T), true);
                        }
                    }
                }

                return _Instance;
            }
        }
    }
}
