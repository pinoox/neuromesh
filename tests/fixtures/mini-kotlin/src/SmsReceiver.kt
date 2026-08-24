package com.example.app

import android.content.BroadcastReceiver
import com.example.app.SmsStore

class SmsReceiver : BroadcastReceiver() {
    override fun onReceive(body: String?) {
        SmsStore.save(body)
    }
}
