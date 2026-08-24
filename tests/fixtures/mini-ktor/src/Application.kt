package com.example.app

import io.ktor.server.application.*
import io.ktor.server.routing.*

fun Application.module() {
    routing {
        post("/sms") { store() }
    }
}

fun store() {
    SmsStore.save("inbox")
}
