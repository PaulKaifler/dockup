import paramiko
import os
import config
import datetime
import logging
import time
import subprocess
from email_service import send_email

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')

# Load the SSH key name from the environment variable
config.SSH_KEY = f"/root/.ssh/{os.getenv('SSH_KEY')}"

def perform_backup():
    backup_summary = []
    
    volumes = []
    for name in os.listdir('/mnt'):
        data_path = os.path.join('/mnt', name, '_data')
        if os.path.isdir(data_path):
            volumes.append((name, data_path))

    # log all available volumes
    logging.info("Available volumes:")
    for volume_name, volume_path in volumes:
        logging.info(f"Volume: {volume_name}")
    if not volumes:
        logging.warning("No volumes found to backup.")
        return
    
    # Check if SSH key exists
    if not os.path.exists(config.SSH_KEY):
        logging.error(f"SSH key not found at {config.SSH_KEY}")
        raise Exception(f"SSH key not found at {config.SSH_KEY}")

    ssh = paramiko.SSHClient()
    ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    try:
        ssh.connect(
            hostname=config.SSH_HOST,
            username=config.SSH_USER,
            key_filename=config.SSH_KEY
        )
    except paramiko.ssh_exception.SSHException as e:
        raise Exception(f"SSH connection failed: {str(e)}")

    logging.info("Starting backup process")
    for volume_name, volume_path in volumes:
        logging.info(f"Processing volume: {volume_name}")
        archive_name = f"{volume_name}_{datetime.datetime.now().strftime('%Y-%m-%d_%H-%M-%S')}.tar.gz"
        
        start_time = time.time()

        # Create a tarball of the volume directly from the mounted path
        logging.info(f"Creating tarball for volume {volume_name} from {volume_path}")
        tarball_command = f"tar czf {archive_name} -C {volume_path} ."
        tarball_result = os.system(tarball_command)

        # Check if tarball was created successfully
        if tarball_result != 0 or not os.path.exists(archive_name):
            logging.error(f"Failed to create tarball for volume {volume_name}. Skipping.")
            continue

        # Create a folder for the volume on the remote system
        remote_folder = f"{config.REMOTE_BACKUP_PATH}/{volume_name}"
        logging.info(f"Creating remote folder: {remote_folder}")
        ssh.exec_command(f"mkdir -p {remote_folder}")

        # Transfer the tarball over SSH
        logging.info(f"Transferring tarball {archive_name} to {remote_folder}")
        sftp = ssh.open_sftp()
        sftp.put(archive_name, f"{remote_folder}/{archive_name}")
        sftp.close()

        end_time = time.time()
        duration = end_time - start_time
        archive_size = subprocess.getoutput(f"du -sh {archive_name} | cut -f1")

        # Get number of backups on remote system
        stdin, stdout, stderr = ssh.exec_command(f"ls {remote_folder}/*.tar.gz 2>/dev/null | wc -l")
        remote_count = stdout.read().decode().strip()

        # Clean up local tarball
        logging.info(f"Cleaning up local tarball: {archive_name}")
        os.remove(archive_name)

        backup_summary.append(f"{volume_name}\n{archive_size} | {duration:.2f}s | {remote_count}")

    logging.info("Backup process completed successfully")

    logging.info("Backup Summary:")
    summary_lines = []
    for entry in backup_summary:
        logging.info(entry)
        summary_lines.append(f"- {entry}")

    plain_body = "Docker Volume Backup Summary\n\nVolume\nSize | Time | Remote Count\n\n" + "\n\n".join(summary_lines)

    send_email("Docker Volume Backup Report", plain_body)

    ssh.close()