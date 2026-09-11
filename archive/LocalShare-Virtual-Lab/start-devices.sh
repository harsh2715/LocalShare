#!/bin/zsh
set -euo pipefail

LAB="/Users/harshsinghal/Desktop/LocalShare-Virtual-Lab"
PROJECT="/Users/harshsinghal/Desktop/Projects/Pairdrop"

mkdir -p "$LAB/device-a/logs" "$LAB/device-b/logs"

env LOCALSHARE_TEST_MODE=1 LOCALSHARE_PROFILE=virtual-a \
  LOCALSHARE_USER_DATA="$LAB/device-a/user-data" \
  LOCALSHARE_BIND_HOST=127.0.0.1 LOCALSHARE_LISTENER_PORT=33201 \
  LOCALSHARE_DEVICE_NAME=Virtual-Mac-A \
  LOCALSHARE_DOWNLOAD_FOLDER="$LAB/device-a/downloads" \
  npm --prefix "$PROJECT" start > "$LAB/device-a/logs/app.log" 2>&1 &
echo $! > "$LAB/device-a/app.pid"

env LOCALSHARE_TEST_MODE=1 LOCALSHARE_PROFILE=virtual-b \
  LOCALSHARE_USER_DATA="$LAB/device-b/user-data" \
  LOCALSHARE_BIND_HOST=127.0.0.1 LOCALSHARE_LISTENER_PORT=33202 \
  LOCALSHARE_DEVICE_NAME=Virtual-Mac-B \
  LOCALSHARE_DOWNLOAD_FOLDER="$LAB/device-b/downloads" \
  npm --prefix "$PROJECT" start > "$LAB/device-b/logs/app.log" 2>&1 &
echo $! > "$LAB/device-b/app.pid"

echo "Started Virtual Device A on 127.0.0.1:33201"
echo "Started Virtual Device B on 127.0.0.1:33202"
