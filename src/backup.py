import docker
import paramiko
import os
import config
import datetime
import logging

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')

def perform_backup():
    client = docker.from_env()
    volumes = client.volumes.list()

    ssh = paramiko.SSHClient()
    ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    try:
        ssh.connect(
            hostname=config.SSH_HOST,
            username=config.SSH_USER,
            key_filename=config.SSH_KEY_PATH
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
        os.system(f"docker run --rm -v {volume_name}:/data -v $(pwd):/backup busybox tar czf /backup/{archive_name} /data")

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