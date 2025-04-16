import yagmail
import config

def send_email(subject, body):
    yag = yagmail.SMTP(config.EMAIL_USER, config.EMAIL_PASSWORD)
    yag.send(to=config.NOTIFY_EMAIL, subject=subject, contents=body)