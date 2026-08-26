<?php

action([MainController::class, 'index'])->name('home');
action([SmsController::class, 'store'])->name('sms.store');
