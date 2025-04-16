import backup
import email_service
import config
import logging

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')

def main():
    logging.info("Starting main backup process")
    try:
        # Perform the backup
        backup.perform_backup()
        logging.info("Backup completed successfully")
    except Exception as e:
        logging.error(f"Backup failed: {str(e)}")
        # Send failure email
        email_service.send_email("Backup Failed", f"An error occurred: {str(e)}")

if __name__ == "__main__":
    main()