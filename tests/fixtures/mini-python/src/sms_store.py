class SmsStore:
    def save(self, body):
        return self.persist(body)

    def persist(self, body):
        return body
