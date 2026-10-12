// Servidor MCP stdio inofensivo para um spike do Atlas. Sem require(): sem fs, sem net, sem child_process.
// Lê linhas JSON-RPC da stdin e responde com dados estáticos. MODE=die sai depois de tools/list;
// NONCE faz uma chamada de ferramenta responder `atlas-spike-called:<NONCE>`, para distinguir um modelo que realmente a chamou de um que chutou.
// MODE=requireenv só inicia se um segredo chegou até ele. MODE=slow espera DELAY_MS antes de responder. MODE=stubborn não sai quando a sua entrada termina; MODE=badschema anuncia uma ferramenta cujo inputSchema não é um esquema JSON válido.
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
      send({
        jsonrpc: '2.0',
        id: m.id,
        result: {
          content: [
            {
              type: 'text',
              text: process.env.NONCE ? 'atlas-spike-called:' + process.env.NONCE : 'static',
            },
          ],
        },
      });
    else if (m.id !== undefined)
      send({ jsonrpc: '2.0', id: m.id, error: { code: -32601, message: 'no' } });
  }
});
// MODE=stubborn ignora o fim da sua entrada, como um servidor que continua rodando por conta própria.
process.stdin.on('end', () => {
  if (mode !== 'stubborn') process.exit(0);
});
if (mode === 'stubborn') setInterval(() => {}, 1000);
// MODE=slow não responde nada até passarem DELAY_MS, como um servidor que precisa se baixar primeiro.
if (mode === 'slow') {
  process.stdin.pause();
  setTimeout(() => process.stdin.resume(), Number(process.env.DELAY_MS || '0'));
}
// MODE=requireenv sai de imediato a menos que a variável nomeada por REPORT_ENV guarde o valor de EXPECT: um
// servidor que só inicia quando um segredo realmente chegou até ele, visível como `connected` ou não.
if (mode === 'requireenv' && process.env[process.env.REPORT_ENV || ''] !== process.env.EXPECT)
  process.exit(3);
