#!/bin/zsh
set -euo pipefail
LAB="/Users/harshsinghal/Desktop/LocalShare-Virtual-Lab"
PROJECT="/Users/harshsinghal/Desktop/Projects/Pairdrop"
mkdir -p "$LAB/device-a/data" "$LAB/device-a/downloads" "$LAB/device-a/temp" "$LAB/device-a/logs"
exec "$PROJECT/target/debug/localshare-test-node" --profile virtual-a --device-name Virtual-Device-A --data-dir "$LAB/device-a/data" --download-dir "$LAB/device-a/downloads" --temp-dir "$LAB/device-a/temp" --bind-host 127.0.0.1 --port 33201 --health-port 34201 --daemon >> "$LAB/device-a/logs/node.log" 2>&1
