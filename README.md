# Dockup
> A fully automatic docker CLI written in RUST

## Install
1. Clone this repo
2. Make sure you have rust installed on your machine
3. `./install`

## Setup
You need to configure the following on first usage. You will be automaticly prompted.
- `DOCKER_PARENT`: Parent folder of all projects folders (here `docker`). `Dockup` expects the following structure:
```
docker
├── project_1
│   ├── docker-compose.yml
│   └── other files ...
├── project_2
└── ...
```
- `REMOTE_BACKUP_PATH`: Location on backup target machine
- `SSH_USER`: User for the backup target machine
- `SSH_HOST`: Host machine
- `SSH_KEY`: Authentication for the backup target machine
- `EMAIL_HOST`:
- `EMAIL_PORT`: 465
- `EMAIL_USER`: 
- `EMAIL_PASSWORD`: 
- `RECEIVER_MAIL`: 

## How does it work
1. On each backup cycle `Dockup` will scan all repos in `DOCKER_PARENT`, extracting all projects.
2. Each detected repository is a new *backup application*
3. For each *backup application* following steps are performed:
   1. Scan `docker-compose` files for used volumes
   2. Add each volume to mount list
4. `Dockup` will mount all volumes that were previously detected
5. For each *backup application* following steps are performed:
   1. Create (if not yet existent) folder on backup target with same name as source
   2. Create new folder with current date and time
   3. Create two folders:
      - REPO
      - VOLUMES
   4. Create tar ball with repo content and copy to target
   5. Create tar ball for each volume (with original name) and copy to target
6. Send job done email


## Recommended crontab
```sh
crontab -e
```

```
5 6,18 * * * /root/dockup/dockup backup
```