import { Controller, Post } from "@nestjs/common";
import { SmsStore } from "./sms_store";

@Controller("sms")
export class SmsController {
  @Post()
  store(body: string) {
    return SmsStore.save(body);
  }
}
