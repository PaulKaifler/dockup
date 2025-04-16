import docker
import paramiko
import os
import config
import datetime

def perform_backup():
    client = docker.from_env()
    volumes = client.volumes.list()

    ssh = paramiko.SSHClient()
    ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    ssh.connect(
        hostname=config.SSH_HOST,
        username=config.SSH_USER,
        key_filename=config.SSH_KEY_PATH
    )

    for volume in volumes:
        volume_name = volume.name
        archive_name = f"{volume_name}_{datetime.datetime.now().strftime('%Y-%m-%d_%H-%M-%S')}.tar.gz"

        # Create a tarball of the volume
        os.system(f"docker run --rm -v {volume_name}:/data -v $(pwd):/backup busybox tar czf /backup/{archive_name} /data")

        # Create a folder for the volume on the remote system
        remote_folder = f"{config.REMOTE_BACKUP_PATH}/{volume_name}"
        ssh.exec_command(f"mkdir -p {remote_folder}")

        # Transfer the tarball over SSH
        sftp = ssh.open_sftp()
        sftp.put(archive_name, f"{remote_folder}/{archive_name}")
        sftp.close()

        # Clean up local tarball
        os.remove(archive_name)

    ssh.close()