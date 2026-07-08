#!/bin/zsh
set -euo pipefail

FOCO_DEV_BACKEND_PORT="${FOCO_DEV_BACKEND_PORT:-33210}"
FOCO_DEV_CONFIG_DIR="${FOCO_DEV_CONFIG_DIR:-$HOME/.foco-dev}"
FOCO_DEV_FRONTEND_PORT="${FOCO_DEV_FRONTEND_PORT:-16000}"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "start-dev.sh is intended for macOS."
  exit 1
fi

SCRIPT_DIR="${0:A:h}"

if lsof -nP -iTCP:"$FOCO_DEV_BACKEND_PORT" -sTCP:LISTEN >/dev/null 2>&1; then
  echo "Backend port $FOCO_DEV_BACKEND_PORT is already in use:"
  lsof -nP -iTCP:"$FOCO_DEV_BACKEND_PORT" -sTCP:LISTEN
  echo "Stop that process or run: FOCO_DEV_BACKEND_PORT=<free-port> $0"
  exit 1
fi

osascript - "$SCRIPT_DIR" "$FOCO_DEV_BACKEND_PORT" "$FOCO_DEV_CONFIG_DIR" "$FOCO_DEV_FRONTEND_PORT" <<'APPLESCRIPT'
on run argv
  set repoRoot to item 1 of argv
  set backendPort to item 2 of argv
  set configDir to item 3 of argv
  set frontendPort to item 4 of argv

  set backendCommand to "cd " & quoted form of repoRoot & " && echo Starting Foco backend on port " & backendPort & " with config " & quoted form of configDir & " && npm run backend -- " & backendPort & " " & quoted form of configDir
  set frontendCommand to "cd " & quoted form of repoRoot & " && echo Starting Foco frontend on port " & frontendPort & " && npm run frontend -- " & backendPort & " " & quoted form of configDir & " " & frontendPort

  tell application "Terminal"
    activate
    do script backendCommand
    do script frontendCommand
  end tell
end run
APPLESCRIPT
