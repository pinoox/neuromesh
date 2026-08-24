package com.example.app

object SmsStore {
    fun save(body: String?) {
        persist(body)
    }

    private fun persist(body: String?) {}
}
