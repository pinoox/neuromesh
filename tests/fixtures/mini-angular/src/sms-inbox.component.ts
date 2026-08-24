import { Component } from "@angular/core";
import { saveSms } from "./sms_store";

@Component({ selector: "sms-inbox", template: "" })
export class SmsInboxComponent {
  store(body: string) {
    return saveSms(body);
  }
}
