namespace App
{
    public class Program
    {
        public static void Main()
        {
            var app = WebApplication.Create();
            app.MapPost("/sms", Store);
        }

        public static void Store(string body)
        {
            SmsStore.Save(body);
        }
    }
}
