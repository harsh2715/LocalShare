#!/bin/zsh
set -euo pipefail
LAB="/Users/harshsinghal/Desktop/LocalShare-Virtual-Lab"
PROJECT="/Users/harshsinghal/Desktop/Projects/Pairdrop"
mkdir -p "$LAB/device-b/data" "$LAB/device-b/downloads" "$LAB/device-b/temp" "$LAB/device-b/logs"
exec "$PROJECT/target/debug/localshare-test-node" --profile virtual-b --device-name Virtual-Device-B --data-dir "$LAB/device-b/data" --download-dir "$LAB/device-b/downloads" --temp-dir "$LAB/device-b/temp" --bind-host 127.0.0.1 --port 33202 --health-port 34202 --daemon >> "$LAB/device-b/logs/node.log" 2>&1
