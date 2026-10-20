#!/usr/bin/env bash
# Runs an `ai-memory` server for Atlas (ADR 0040) with the settings Atlas expects:
#   - only on this machine (127.0.0.1)
#   - no embedding model download (AI_MEMORY_EMBEDDING_PROVIDER=none)
#   - no import of your agents' session history (AI_MEMORY_BACKFILL_ON_START=false)
#   - no hooks, MCP entries or services installed anywhere (it never calls setup/install-*)
#
# Usage: scripts/ai-memory-local.sh [--data-dir DIR] [--port PORT] [--bin PATH]
#   --data-dir  where the server keeps its notes (default: ~/.atlas/ai-memory)
#   --port      loopback port (default: 49374)
#   --bin       the ai-memory binary (default: `ai-memory` on PATH)
#
# Then, in Atlas: Settings > Memory > External memory server:
#   address http://127.0.0.1:PORT, data folder DIR (absolute).
set -euo pipefail

data_dir="$HOME/.atlas/ai-memory"
port=49374
bin="ai-memory"

while [ $# -gt 0 ]; do
  case "$1" in
    --data-dir) data_dir="${2:?--data-dir needs a folder}"; shift 2 ;;
    --port) port="${2:?--port needs a number}"; shift 2 ;;
    --bin) bin="${2:?--bin needs a path}"; shift 2 ;;
    -h|--help) sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

case "$data_dir" in /*) ;; *) echo "--data-dir must be an absolute path" >&2; exit 2 ;; esac
case "$port" in ''|*[!0-9]*) echo "--port must be a number" >&2; exit 2 ;; esac
command -v "$bin" >/dev/null 2>&1 || [ -x "$bin" ] || { echo "ai-memory not found: $bin" >&2; exit 1; }

# A port that something already holds could belong to anything: the token would go to it.
if (exec 3<>"/dev/tcp/127.0.0.1/$port") 2>/dev/null; then
  echo "port $port on 127.0.0.1 is already in use; pick another with --port" >&2
  exit 1
fi

export AI_MEMORY_EMBEDDING_PROVIDER=none
export AI_MEMORY_BACKFILL_ON_START=false
export AI_MEMORY_BIND="127.0.0.1:$port"
export AI_MEMORY_SERVER_URL="http://127.0.0.1:$port"

# Only a folder this script creates is made private: a folder that was there is not ours to chmod
# (--data-dir "$HOME" must never change the permissions of the home folder).
if [ ! -d "$data_dir" ]; then
  mkdir -p "$data_dir"
  chmod 700 "$data_dir"
fi
[ -f "$data_dir/config.toml" ] || "$bin" --data-dir "$data_dir" init >/dev/null

echo "ai-memory for Atlas: http://127.0.0.1:$port  data folder: $data_dir"
# --bind is given explicitly too, so the loopback address does not rest on the environment alone
exec "$bin" --data-dir "$data_dir" serve --transport http --bind "127.0.0.1:$port"
