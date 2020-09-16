using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Web.Script.Serialization;

namespace FreeControl.Utils
{
    public class JsonHelper
    {
        /// <summary>  
        /// json反序列化  
        /// </summary>  
        /// <param name="input"></param>  
        /// <returns></returns>  
        public static T jsonDes<T>(string input)
        {
            JavaScriptSerializer jss = new JavaScriptSerializer();
            return jss.Deserialize<T>(input);
        }
        public static string json(object obj)
        {
            JavaScriptSerializer jss = new JavaScriptSerializer();
            return jss.Serialize(obj);
        }
    }
}
