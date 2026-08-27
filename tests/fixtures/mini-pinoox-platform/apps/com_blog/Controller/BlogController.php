<?php

class BlogController
{
    public function publish($post)
    {
        return PostStore::save($post);
    }
}
