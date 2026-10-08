// Harmless MCP stdio server for an Atlas spike. No require(): no fs, no net, no child_process.
// Reads JSON-RPC lines from stdin and answers with static data. MODE=die exits after tools/list;
// MODE=stubborn does not exit when its input ends; MODE=badschema advertises a tool whose inputSchema is not a valid JSON schema.
const mode = process.env.MODE || 'ok';
const send = (o) => process.stdout.write(JSON.stringify(o) + '\n');
const tools = [
  {
    name: 'echo_static',
    description: 'Returns a fixed string. Spike only.',
    inputSchema: { type: 'object', properties: { text: { type: 'string' } } },
  },
];
if (process.env.REPORT_ENV)
  tools.push({
    name:
      'env_' +
      String(process.env[process.env.REPORT_ENV] ?? 'unset').replace(/[^A-Za-z0-9_]/g, '_'),
    description: 'reports an env var',
    inputSchema: { type: 'object' },
  });
if (mode === 'grow')
  tools.push({ name: 'extra_tool', description: 'added later', inputSchema: { type: 'object' } });
if (mode === 'badschema')
  tools.push({ name: 'broken', description: 'bad schema', inputSchema: 'not-a-schema' });
let buf = '';
process.stdin.on('data', (d) => {
  buf += d;
  let i;
  while ((i = buf.indexOf('\n')) >= 0) {
    const line = buf.slice(0, i).trim();
    buf = buf.slice(i + 1);
    if (!line) continue;
    let m;
    try {
      m = JSON.parse(line);
    } catch {
      continue;
    }
    if (m.method === 'initialize')
      send({
        jsonrpc: '2.0',
        id: m.id,
        result: {
          protocolVersion: m.params?.protocolVersion || '2025-06-18',
          capabilities: { tools: {} },
          serverInfo: { name: 'atlas-spike', version: '0.0.1' },
        },
      });
    else if (m.method === 'tools/list') {
      send({ jsonrpc: '2.0', id: m.id, result: { tools } });
      if (mode === 'die') setTimeout(() => process.exit(0), 50);
    } else if (m.method === 'tools/call')
      send({ jsonrpc: '2.0', id: m.id, result: { content: [{ type: 'text', text: 'static' }] } });
    else if (m.id !== undefined)
      send({ jsonrpc: '2.0', id: m.id, error: { code: -32601, message: 'no' } });
  }
});
// MODE=stubborn ignores the end of its input, like a server that keeps running on its own.
process.stdin.on('end', () => {
  if (mode !== 'stubborn') process.exit(0);
});
if (mode === 'stubborn') setInterval(() => {}, 1000);
