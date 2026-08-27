<?php

use function Pinoox\Router\post;

post('/publish')->action([BlogController::class, 'publish'])->name('blog.publish');
