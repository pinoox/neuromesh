import 'sms_store.dart';
import 'package:flutter/material.dart';

class SmsInbox extends StatelessWidget {
  void onReceive(String body) {
    SmsStore.save(body);
  }
}
