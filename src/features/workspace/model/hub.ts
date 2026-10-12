import type { McpConnectionDto, McpGrantDto, McpServerStatusDto } from '@/lib/tauri/commands';

/** What a connection looks like in the hub: the last examination, and whether it is on at all. */
export type HubStatus = 'off' | 'unexamined' | McpServerStatusDto;

export function statusOf(connection: McpConnectionDto): HubStatus {
  if (!connection.enabled) return 'off';
  return connection.discovery?.status ?? 'unexamined';
}

export interface HubMetrics {
  total: number;
  connected: number;
  degraded: number;
  off: number;
  tools: number;
  grants: number;
}

/**
 * Counts what the core reported and nothing it did not: "connected" is the last examination, not a
 * live ping, and a server nobody examined is in no bucket but the total.
 */
export function metricsOf(connections: McpConnectionDto[], grants: McpGrantDto[]): HubMetrics {
  const on = connections.filter((c) => c.enabled);
  return {
    total: connections.length,
    connected: on.filter((c) => c.discovery?.status === 'connected').length,
    degraded: on.filter(
      (c) => c.discovery?.status === 'failed' || c.discovery?.status === 'needs_auth',
    ).length,
    off: connections.length - on.length,
    tools: on.reduce((sum, c) => sum + (c.discovery?.tools.length ?? 0), 0),
    grants: grants.filter((g) => connections.some((c) => c.id === g.connectionId)).length,
  };
}

export type HubTab = 'all' | 'local' | 'remote';

export interface HubFilter {
  tab: HubTab;
  query: string;
  onlyConnected: boolean;
}

/** The command line as text, the way a card shows it. Empty for a transport Atlas does not run. */
export function commandLine(connection: McpConnectionDto): string {
  return connection.transport.kind === 'stdio'
    ? [connection.transport.executable, ...(connection.transport.args ?? [])].join(' ')
    : connection.transport.url;
}

export function matches(connection: McpConnectionDto, filter: HubFilter): boolean {
  const remote = connection.transport.kind !== 'stdio';
  if (filter.tab === 'local' && remote) return false;
  if (filter.tab === 'remote' && !remote) return false;
  if (filter.onlyConnected && statusOf(connection) !== 'connected') return false;
  const needle = filter.query.trim().toLowerCase();
  if (needle === '') return true;
  const haystack = [
    connection.name,
    commandLine(connection),
    ...(connection.discovery?.tools ?? []),
  ]
    .join(' ')
    .toLowerCase();
  return haystack.includes(needle);
}

/** `ghp_…` is a secret; a plain value is shown as it is. */
export function envOf(connection: McpConnectionDto) {
  return connection.transport.kind === 'stdio' ? (connection.transport.env ?? []) : [];
}

/**
 * The connection as the JSON Atlas starts it from. A secret is its name and whether a value is
 * stored; never the value (Atlas does not have it in the configuration).
 */
export function manifestOf(connection: McpConnectionDto): string {
  if (connection.transport.kind !== 'stdio') return JSON.stringify(connection.transport, null, 2);
  const { executable, args, env } = connection.transport;
  return JSON.stringify(
    {
      command: executable,
      args: args ?? [],
      env: Object.fromEntries(
        (env ?? []).map((e) => [
          e.name,
          e.value.kind === 'plain'
            ? e.value.value
            : connection.secrets[e.name] === undefined
              ? '<secret: not stored>'
              : '<secret: stored in the system credential store>',
        ]),
      ),
    },
    null,
    2,
  );
}
