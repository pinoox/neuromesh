<?php

namespace App\Http\Controllers;

use App\Models\SmsMessage;

class SmsController
{
    public function store(string $body): SmsMessage
    {
        return SmsMessage::query()->create(['body' => $body]);
    }
}
