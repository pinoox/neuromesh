import { saveSms } from '../../lib/sms_store';

export async function POST(body: string) {
  return saveSms(body);
}
