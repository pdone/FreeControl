using System;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Reflection;
using System.Threading;

namespace FreeControl.Utils
{
    public class Logger
    {
        //public static string path = Path.Combine(System.Environment.GetFolderPath(System.Environment.SpecialFolder.ApplicationData), "Logs");

        public static string path = Path.Combine(Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location), "logs");

        /// <summary>
        /// 启用日志
        /// </summary>
        public static bool enable = true;

        //死锁
        public static object loglock = new object();

        public static void Debug(string content)
        {
            WriteLog("DEBUG", content);
        }

        public static void Info(string content)
        {
            WriteLog("INFO", content);
        }

        public static void Info(string content, string type)
        {
            WriteLog(type, content);
        }

        public static void Error(string content)
        {
            WriteLog("ERROR", content);
        }

        public static void Error(Exception ex)
        {
            WriteLog("ERROR", ex.ToString());
        }

        protected static void WriteLog(string type, string content)
        {
            if (!enable)
            {
                return;
            }
            lock (loglock)
            {
                if (!Directory.Exists(path))//如果日志目录不存在就创建
                {
                    Directory.CreateDirectory(path);
                }

                string time = DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss:fff");//获取当前系统时间
                string filename = path + "/" + DateTime.Now.ToString("yyyy-MM-dd") + ".log";//用日期对日志文件命名

                //创建或打开日志文件，向日志文件末尾追加记录
                StreamWriter mySw = File.AppendText(filename);

                //向日志文件写入内容
                string write_content = time + " [" + type + "] " + content;
                mySw.WriteLine(write_content);

                //关闭日志文件
                mySw.Close();
            }
        }
    }

    public class Logger2
    {
        /// <summary>
        /// 日志文件名前缀
        /// </summary>
        public const string BaseFileName = "log_";

        /// <summary>
        /// 保存路径
        /// </summary>
        public static string ServerPath = AppDomain.CurrentDomain.BaseDirectory; //当前BIN目录 
        public static string LogPath
        {
            get
            {
                if (!File.Exists(ServerPath + "log"))
                    Directory.CreateDirectory(ServerPath + "log");
                return ServerPath + "log" + "\\" + BaseFileName + DateTime.Now.Month + "_" + DateTime.Now.Day + "_" + DateTime.Now.Year + ".log";
            }
        }

        /// <summary>
        /// 填写日志信息
        /// </summary>
        /// <param name="message">内容</param>
        private static void LogMessage(string message)
        {
            StreamWriter writer = null;

            try
            {
                FormatMessage(ref message);

                writer = File.AppendText(LogPath);
                writer.WriteLine(message);
            }
            catch (Exception ex)
            {
                System.Diagnostics.Debug.WriteLine(ex.Message);
            }
            finally
            {
                if (null != writer)
                {
                    writer.Close();
                }
            }

        }

        /// <summary>
        /// 格式化信息
        /// </summary>
        /// <param name="message"></param>
        private static void FormatMessage(ref string message)
        {
            int CurrentProcessId = Process.GetCurrentProcess().Id;
            int CurrentThreadId = Thread.CurrentThread.ManagedThreadId;
            string temp = message;
            string format = "yyyy-MM-dd HH:mm:ss.fff";
            message = String.Format(CultureInfo.InvariantCulture, "{0} [pid:{1}] [tid:{2}] - {3}",
                DateTime.Now.ToString(format, CultureInfo.InvariantCulture),
                CurrentProcessId,
                CurrentThreadId,
                temp
                );
        }

        /// <summary>
        /// 对外信息接口
        /// </summary>
        /// <param name="format"></param>
        /// <param name="args"></param>
        public static void Info(string format, params object[] args)
        {
            Logger2.Info(string.Format(format, args));
        }

        /// <summary>
        /// 对外信息接口
        /// </summary>
        /// <param name="message"></param>
        public static void Info(string message)
        {
            Logger2.LogMessage(message);
        }

        /// <summary>
        /// 对外信息接口
        /// </summary>
        /// <param name="message"></param>
        public static void Error(string message)
        {
            Logger2.LogMessage(message);
        }

        /// <summary>
        /// 对外信息接口
        /// </summary>
        /// <param name="format"></param>
        /// <param name="args"></param>
        public static void Error(string format, params object[] args)
        {
            Logger2.Error(string.Format(format, args));
        }
    }
}
