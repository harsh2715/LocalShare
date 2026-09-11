#!/bin/zsh
set -u
LAB="/Users/harshsinghal/Desktop/LocalShare-Virtual-Lab"
for pid_file in "$LAB/device-a/node.pid" "$LAB/device-b/node.pid"; do
  if [[ -f "$pid_file" ]]; then
    pid="$(<$pid_file)"
    if ps -p "$pid" -o command= | grep -q localshare-test-node; then
      kill "$pid" 2>/dev/null || true
    fi
    rm -f "$pid_file"
  fi
done
for port in 33201 33202; do
  pid=$(lsof -nP -t -iTCP:"$port" -sTCP:LISTEN 2>/dev/null | head -n 1 || true)
  if [[ -n "$pid" ]]; then kill "$pid" 2>/dev/null || true; fi
done
echo "Stop requested for both virtual nodes."
