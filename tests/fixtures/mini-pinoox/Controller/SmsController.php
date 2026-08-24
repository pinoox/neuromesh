<?php

class SmsController
{
    public function store($body)
    {
        SmsStore::save($body);
        return $body;
    }
}
