# Dockup: Docker Volume Backup Tool

Dockup is a Python-based tool designed to back up Docker volumes to a remote server using SSH. It also provides email notifications for backup success or failure.

## Features
- Automatically detects Docker volumes for backup.
- Compresses volumes into tarballs.
- Transfers backups to a remote server via SSH.
- Sends email notifications with backup summaries.

## Prerequisites
- Docker and Docker Compose installed.
- A remote server accessible via SSH.
- Python 3.10 or later.

## Setup

1. Clone the repository:
   ```bash
   git clone <repository-url>
   cd dockup
   ```

2. Configure the `.env` file:
   Update the `.env` file with your SSH and email credentials. Ensure the file is structured as follows:
   ```env
   # SSH Configuration
   SSH_HOST=<your-ssh-host>
   SSH_USER=<your-ssh-user>
   REMOTE_BACKUP_PATH=<remote-backup-path>
   SSH_KEY=<ssh-key-name>

   # Email Configuration
   EMAIL_HOST=<email-host>
   EMAIL_PORT=<email-port>
   EMAIL_USER=<email-user>
   EMAIL_PASSWORD=<email-password>
   NOTIFY_EMAIL=<notification-email>
   ```

3. Build the Docker image:
   ```bash
   docker-compose build
   ```

4. Start the service:
   ```bash
   docker-compose up -d
   ```

## Using with Portainer

Portainer is a lightweight management UI that allows you to easily manage your Docker environments. To use Dockup with Portainer:

1. **Deploy the Stack via Repository:**
   - Log in to your Portainer instance.
   - Navigate to the "Stacks" section.
   - Click on "Add Stack."
   - Provide a name for the stack (e.g., `dockup`).
   - Select the "Repository" option.
   - Enter the Git repository URL for this project.
   - Add env variables.
   - Click "Deploy the stack."

2. **Verify the Service:**
   - Go to the "Containers" section in Portainer.
   - Ensure the `dockup` container is running.

3. **Monitor Logs:**
   - Click on the `dockup` container.
   - Navigate to the "Logs" tab to monitor the backup process.

## Backup Process
Dockup runs a daily backup process by default. It:
1. Detects Docker volumes mounted at `/mnt`.
2. Creates tarballs of the volumes.
3. Transfers the tarballs to the remote server.
4. Sends an email summary of the backup process.

## Troubleshooting
- **SSH Key Not Found:** Ensure the SSH key specified in the `.env` file exists in the `~/.ssh` directory.
- **Email Authentication Failed:** Verify the email credentials in the `.env` file.
- **No Volumes Found:** Ensure Docker volumes are mounted at `/var/lib/docker/volumes`.

## License
This project is licensed under the MIT License. See the LICENSE file for details.