<?php

use App\Http\Controllers\SmsController;
use Illuminate\Support\Facades\Route;

Route::post('/sms', [SmsController::class, 'store'])->name('sms.store');
Route::resource('inbox', SmsController::class);
