<?php

class ShopController
{
    public function checkout($order)
    {
        return OrderStore::save($order);
    }
}
