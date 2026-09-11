#!/bin/zsh
set -euo pipefail
LAB="/Users/harshsinghal/Desktop/LocalShare-Virtual-Lab"
mkdir -p "$LAB/device-a/logs" "$LAB/device-b/logs"
nohup "$LAB/scripts/start-a.sh" > "$LAB/device-a/logs/launcher.log" 2>&1 < /dev/null &
while ! lsof -nP -t -iTCP:33201 -sTCP:LISTEN >/dev/null 2>&1; do sleep 0.1; done
echo "$(lsof -nP -t -iTCP:33201 -sTCP:LISTEN | head -n 1)" > "$LAB/device-a/node.pid"
nohup "$LAB/scripts/start-b.sh" > "$LAB/device-b/logs/launcher.log" 2>&1 < /dev/null &
while ! lsof -nP -t -iTCP:33202 -sTCP:LISTEN >/dev/null 2>&1; do sleep 0.1; done
echo "$(lsof -nP -t -iTCP:33202 -sTCP:LISTEN | head -n 1)" > "$LAB/device-b/node.pid"
echo "Virtual Device A and B started."
