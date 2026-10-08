#!/bin/zsh
# Zero-cost probe of what `claude -p` loads with an Atlas-generated MCP config: the model does not
# exist, so the CLI prints its `init` message and fails at the first request. Nothing is spent.
# usage: SPIKE=<scratch dir> run.sh <label> <ok|die|badschema> <extra claude args...>
# Create $SPIKE/mcp.json first, e.g.
#   {"mcpServers":{"atlasspike":{"command":"node","args":["<path>/echo-mcp.js"]}}}
label=$1; mode=$2; shift 2
cd /tmp
MODE=$mode claude -p --output-format stream-json --verbose --include-partial-messages \
  --model not-a-real-model-xyz --no-session-persistence --disable-slash-commands "$@" -- ping \
  > "$SPIKE/out-$label.jsonl" 2> "$SPIKE/err-$label.txt"
echo "$label exit=$?"
jq -c 'select(.type=="system" and .subtype=="init") | {tools, mcp_servers, skills: (.skills|length), plugins: (.plugins|length)}' "$SPIKE/out-$label.jsonl" | head -3
