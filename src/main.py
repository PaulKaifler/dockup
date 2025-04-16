import backup
import email_service
import config

def main():
    try:
        # Perform the backup
        backup.perform_backup()
        # Send success email
        email_service.send_email("Backup Successful", "The backup was completed successfully.")
    except Exception as e:
        # Send failure email
        email_service.send_email("Backup Failed", f"An error occurred: {str(e)}")

if __name__ == "__main__":
    main()