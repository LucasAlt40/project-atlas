import type { McpConnectionDto } from '@/lib/tauri/commands';
import { commandLine, manifestOf, matches, metricsOf, statusOf } from './hub';

const connection = (over: Partial<McpConnectionDto> = {}): McpConnectionDto => ({
  id: 'c1',
  workspaceId: 'w1',
  name: 'files',
  transport: { kind: 'stdio', executable: 'node', args: ['server.js'] },
  enabled: true,
  required: false,
  secrets: {},
  createdAt: 1,
  ...over,
});
const found = (status: 'connected' | 'failed' | 'needs_auth', tools: string[] = []) => ({
  discoveredAt: 1,
  runtimeId: 'claude',
  status,
  tools,
});

describe('the hub model', () => {
  it('a switched-off connection is off whatever it was last seen as', () => {
    expect(statusOf(connection({ enabled: false, discovery: found('connected') }))).toBe('off');
    expect(statusOf(connection())).toBe('unexamined');
    expect(statusOf(connection({ discovery: found('failed') }))).toBe('failed');
  });

  it('counts only what was reported: unexamined servers are in the total and nowhere else', () => {
    const list = [
      connection({ id: 'a', discovery: found('connected', ['x', 'y']) }),
      connection({ id: 'b', discovery: found('needs_auth') }),
      connection({ id: 'c' }),
      connection({ id: 'd', enabled: false, discovery: found('connected', ['z']) }),
    ];

    expect(
      metricsOf(list, [
        {
          id: 'g',
          connectionId: 'a',
          agentId: null,
          workflowId: null,
          nodeId: null,
          tools: { kind: 'server' },
        },
        {
          id: 'h',
          connectionId: 'gone',
          agentId: null,
          workflowId: null,
          nodeId: null,
          tools: { kind: 'server' },
        },
      ]),
    ).toEqual({ total: 4, connected: 1, degraded: 1, off: 1, tools: 2, grants: 1 });
  });

  it('filters by kind, by what is typed (name, command, tool) and by connected only', () => {
    const a = connection({ discovery: found('connected', ['read_file']) });
    const b = connection({ id: 'b', name: 'web' });

    expect(matches(a, { tab: 'all', query: 'READ_', onlyConnected: false })).toBe(true);
    expect(matches(a, { tab: 'all', query: 'server.js', onlyConnected: false })).toBe(true);
    expect(matches(b, { tab: 'all', query: '', onlyConnected: true })).toBe(false);
    expect(matches(a, { tab: 'remote', query: '', onlyConnected: false })).toBe(false);
    expect(commandLine(a)).toBe('node server.js');
  });

  it('the manifest names a secret and never has its value', () => {
    const withSecret = connection({
      secrets: { API_TOKEN: 5 },
      transport: {
        kind: 'stdio',
        executable: 'node',
        env: [
          { name: 'API_TOKEN', value: { kind: 'secret' } },
          { name: 'REGION', value: { kind: 'plain', value: 'eu' } },
          { name: 'OTHER', value: { kind: 'secret' } },
        ],
      },
    });

    const manifest = JSON.parse(manifestOf(withSecret)) as { env: Record<string, string> };

    expect(manifest.env.REGION).toBe('eu');
    expect(manifest.env.API_TOKEN).toMatch(/stored/);
    expect(manifest.env.OTHER).toMatch(/not stored/);
  });
});
