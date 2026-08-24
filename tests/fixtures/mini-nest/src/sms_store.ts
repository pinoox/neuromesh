export class SmsStore {
  static save(body: string) {
    persist(body);
    return body;
  }
}

function persist(body: string) {}
