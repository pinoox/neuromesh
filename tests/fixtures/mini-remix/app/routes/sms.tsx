import { saveSms } from "../../lib/sms_store";

export async function action(body: string) {
  return saveSms(body);
}
