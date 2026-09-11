#!/bin/zsh
set -u
LAB="/Users/harshsinghal/Desktop/LocalShare-Virtual-Lab"
for pid_file in "$LAB/device-a/app.pid" "$LAB/device-b/app.pid"; do
  if [[ -f "$pid_file" ]]; then
    pid=$(<"$pid_file")
    kill "$pid" 2>/dev/null || true
  fi
done
echo "Stop signal sent to both virtual devices."
