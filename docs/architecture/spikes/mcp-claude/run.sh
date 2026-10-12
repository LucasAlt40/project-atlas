#!/bin/zsh
# Sonda de custo zero do que o `claude -p` carrega com uma configuração MCP gerada pelo Atlas: o modelo não
# existe, então a CLI imprime a mensagem `init` e falha na primeira requisição. Nada é gasto.
# uso: SPIKE=<pasta descartável> run.sh <rótulo> <ok|die|badschema> <argumentos extras do claude...>
# Crie antes o $SPIKE/mcp.json, por exemplo:
#   {"mcpServers":{"atlasspike":{"command":"node","args":["<path>/echo-mcp.js"]}}}
label=$1; mode=$2; shift 2
cd /tmp
MODE=$mode claude -p --output-format stream-json --verbose --include-partial-messages \
  --model not-a-real-model-xyz --no-session-persistence --disable-slash-commands "$@" -- ping \
  > "$SPIKE/out-$label.jsonl" 2> "$SPIKE/err-$label.txt"
echo "$label exit=$?"
jq -c 'select(.type=="system" and .subtype=="init") | {tools, mcp_servers, skills: (.skills|length), plugins: (.plugins|length)}' "$SPIKE/out-$label.jsonl" | head -3
