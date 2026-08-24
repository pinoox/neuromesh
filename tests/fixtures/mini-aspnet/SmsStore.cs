namespace App
{
    public class SmsStore
    {
        public static void Save(string body)
        {
            Persist(body);
        }

        static void Persist(string body) {}
    }
}
