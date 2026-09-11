#!/bin/zsh
set -euo pipefail
for port in 34201 34202; do echo "health $port"; curl --fail --silent --show-error "http://127.0.0.1:$port" || true; echo; done
for port in 33201 33202; do lsof -nP -iTCP:"$port" -sTCP:LISTEN || true; done
