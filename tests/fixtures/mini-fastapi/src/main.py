from fastapi import FastAPI
from sms_store import SmsStore

app = FastAPI()


@app.post("/sms")
def store(body: str):
    SmsStore.save(body)
    return body
