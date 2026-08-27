<?php

namespace Database\Seeders;

use App\Models\SmsMessage;
use Illuminate\Database\Seeder;

class SmsSeeder extends Seeder
{
    public function run(): void
    {
        SmsMessage::factory()->create(['body' => 'hello']);
    }
}
