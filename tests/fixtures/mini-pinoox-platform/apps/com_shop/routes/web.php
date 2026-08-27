<?php

use function Pinoox\Router\post;

post('/checkout')->action([ShopController::class, 'checkout'])->name('shop.checkout');
