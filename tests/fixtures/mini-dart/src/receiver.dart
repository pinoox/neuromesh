import 'sms_store.dart';

void onReceive(String body) {
  SmsStore.save(body);
}
