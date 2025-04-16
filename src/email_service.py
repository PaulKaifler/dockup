import yagmail
import config
import logging

def send_email(subject, body):
    logging.info(f"Sending email to {config.NOTIFY_EMAIL} with subject: {subject}")
    try:
        yag = yagmail.SMTP(config.EMAIL_USER, config.EMAIL_PASSWORD)
        yag.send(to=config.NOTIFY_EMAIL, subject=subject, contents=body)
        logging.info("Email sent successfully")
    except yagmail.error.YagAddressError as e:
        logging.error(f"Invalid email address: {str(e)}")
        raise Exception(f"Invalid email address: {str(e)}")
    except yagmail.error.YagConnectionClosed as e:
        logging.error(f"Email connection failed: {str(e)}")
        raise Exception(f"Email connection failed: {str(e)}")
    except yagmail.error.YagAuthenticationError as e:
        logging.error(f"Email authentication failed: {str(e)}")
        raise Exception(f"Email authentication failed: {str(e)}")