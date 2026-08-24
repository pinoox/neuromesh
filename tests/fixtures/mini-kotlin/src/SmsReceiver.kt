package com.example.app

import com.example.app.SmsStore

class SmsReceiver {
    fun onReceive(body: String?) {
        SmsStore.save(body)
    }
}
