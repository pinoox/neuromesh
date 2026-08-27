<?php

use function Pinoox\Router\{get, post};

get('/')->action([MainController::class, 'index'])->name('home');
post('/sms')->action([SmsController::class, 'store'])->name('sms.store');
action([SmsController::class, 'store'])->name('sms.store.alias');
