using System;
using System.IO;
namespace FreeControl.Utils
{
    public class LogHelper
    {
        //在网站根目录下创建日志目录(bin文件夹→debug文件夹→logs文件夹)
        public static string path = AppDomain.CurrentDomain.BaseDirectory + "logs";

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
                string write_content = time + " " + type + " " + content;
                mySw.WriteLine(write_content);

                //关闭日志文件
                mySw.Close();
            }
        }
    }

}
