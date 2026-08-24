namespace App
{
    class SmsReceiver
    {
        public void OnReceive(string body)
        {
            SmsStore.Save(body);
        }
    }
}
