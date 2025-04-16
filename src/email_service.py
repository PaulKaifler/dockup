import smtplib
from email.mime.text import MIMEText
from email.mime.multipart import MIMEMultipart
import config
import logging

def send_email(subject, body):
    logging.info(f"Sending email to {config.NOTIFY_EMAIL} with subject: {subject}")
    try:
        # Create the email message
        msg = MIMEMultipart()
        msg['From'] = config.EMAIL_USER
        msg['To'] = config.NOTIFY_EMAIL
        msg['Subject'] = subject
        msg.attach(MIMEText(body, 'plain'))

        # Connect to the SMTP server and send the email
        with smtplib.SMTP(config.EMAIL_HOST, config.EMAIL_PORT) as server:
            server.starttls()
            server.login(config.EMAIL_USER, config.EMAIL_PASSWORD)
            server.send_message(msg)

        logging.info("Email sent successfully")
    except smtplib.SMTPAuthenticationError as e:
        logging.error(f"Email authentication failed: {str(e)}")
        raise Exception(f"Email authentication failed: {str(e)}")
    except smtplib.SMTPException as e:
        logging.error(f"Failed to send email: {str(e)}")
        raise Exception(f"Failed to send email: {str(e)}")