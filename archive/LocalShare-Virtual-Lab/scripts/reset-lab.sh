#!/bin/zsh
set -euo pipefail
LAB="/Users/harshsinghal/Desktop/LocalShare-Virtual-Lab"
"$LAB/scripts/stop-all.sh" || true
for node in a b; do rm -rf "$LAB/device-$node/data" "$LAB/device-$node/downloads" "$LAB/device-$node/temp" "$LAB/device-$node/logs"; mkdir -p "$LAB/device-$node/data" "$LAB/device-$node/downloads" "$LAB/device-$node/temp" "$LAB/device-$node/logs"; done
echo "Virtual lab state reset."
