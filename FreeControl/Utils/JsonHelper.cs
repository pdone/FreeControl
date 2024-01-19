using System.Web.Script.Serialization;

namespace FreeControl.Utils
{
    public class JsonHelper
    {
        /// <summary>
        /// Json字符串转对象
        /// </summary>
        /// <typeparam name="T"></typeparam>
        /// <param name="input"></param>
        /// <returns></returns>
        public static T Str2Obj<T>(string input)
        {
            return new JavaScriptSerializer().Deserialize<T>(input);
        }

        /// <summary>
        /// 对象转Json字符串
        /// </summary>
        /// <param name="obj"></param>
        /// <returns></returns>
        public static string Obj2Str(object obj)
        {
            return new JavaScriptSerializer().Serialize(obj);
        }
    }
}
