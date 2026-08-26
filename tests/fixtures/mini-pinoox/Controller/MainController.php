<?php

class MainController extends Controller
{
    public function index()
    {
        return View::render('hello', [
            'title' => 'Pinoox App',
            'message' => 'hello from the starter',
        ]);
    }
}
