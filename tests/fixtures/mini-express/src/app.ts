import express from "express";
import { saveSms } from "./sms_store";

const app = express();

function store(body: string) {
  return saveSms(body);
}

app.post("/sms", (req) => store(String(req.body)));
