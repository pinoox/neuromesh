<?php

namespace Database\Factories;

use App\Models\SmsMessage;
use Illuminate\Database\Eloquent\Factories\Factory;

class SmsMessageFactory extends Factory
{
    protected $model = SmsMessage::class;

    public function definition(): array
    {
        return [
            'body' => fake()->sentence(),
        ];
    }
}
