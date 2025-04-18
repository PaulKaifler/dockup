#!/bin/sh

CONFIG_DIR="/dockup/config"

# Ensure config dir exists
mkdir -p "$CONFIG_DIR"
export DOCKUP_CONFIG_PATH="$CONFIG_DIR/config.json"

if [ "$MODE" = "schedule" ]; then
  echo "$CRON_SCHEDULE dockup backup >> /var/log/cron.log 2>&1" > /etc/cron.d/dockup-cron
  chmod 0644 /etc/cron.d/dockup-cron
  crontab /etc/cron.d/dockup-cron
  touch /var/log/cron.log
  echo "🕒 Starting cron..."
  cron -f
else
  echo "▶️ Running once: dockup backup"
  dockup backup
fi