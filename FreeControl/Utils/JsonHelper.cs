//using Newtonsoft.Json;
using System;
using System.Collections.Generic;
using System.IO;
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
            //return JsonConvert.DeserializeObject<T>(input);
            JavaScriptSerializer javaScriptSerializer = new JavaScriptSerializer();
            return javaScriptSerializer.Deserialize<T>(input);
        }
        public static string json(object obj)
        {
            //return JsonConvert.SerializeObject(obj, Formatting.Indented);
            JavaScriptSerializer javaScriptSerializer = new JavaScriptSerializer();
            return javaScriptSerializer.Serialize(obj);
        }
    }
}
