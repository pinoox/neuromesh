from sms_store import SmsStore


def on_receive(body):
    SmsStore.save(body)
    return body
