import docker
import paramiko
import os
import config
import datetime
import logging

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')

# Load the SSH key name from the environment variable
config.SSH_KEY = f"/root/.ssh/{os.getenv('SSH_KEY')}"

def perform_backup():
    client = docker.from_env()
    volumes = client.volumes.list()

    # log all available volumes
    logging.info("Available volumes:")
    for volume in volumes:
        logging.info(f"Volume: {volume.name}")
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
    for volume in volumes:
        volume_name = volume.name
        logging.info(f"Processing volume: {volume_name}")
        archive_name = f"{volume_name}_{datetime.datetime.now().strftime('%Y-%m-%d_%H-%M-%S')}.tar.gz"

        # Create a tarball of the volume
        logging.info(f"Creating tarball for volume {volume_name}")
        tarball_command = f"docker run --rm -v {volume_name}:/data -v $(pwd):/backup busybox tar czf /backup/{archive_name} /data"
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

        # Clean up local tarball
        logging.info(f"Cleaning up local tarball: {archive_name}")
        os.remove(archive_name)

    logging.info("Backup process completed successfully")

    ssh.close()