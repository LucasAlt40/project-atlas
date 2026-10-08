import { useCallback, useEffect, useState } from 'react';
import type { McpOverviewDto, McpPresetDto } from '@/lib/tauri/commands';
import { listMcpCatalog, listMcpConnections } from '../services/mcpService';
import type { Loadable } from './useSecurity';

export interface McpState {
  overview: McpOverviewDto;
  catalog: McpPresetDto[];
}

/**
 * The workspace's MCP connections, grants and the catalogue of integrations, read from the core
 * every time something changes. The UI keeps no copy of its own to decide from.
 */
export function useMcpConnections(workspaceId: string) {
  const [state, setState] = useState<{ key: string; result: Loadable<McpState> }>();
  const [version, setVersion] = useState(0);
  useEffect(() => {
    let cancelled = false;
    Promise.all([listMcpConnections(workspaceId), listMcpCatalog()])
      .then(([overview, catalog]) => {
        if (!cancelled)
          setState({ key: workspaceId, result: { status: 'ready', value: { overview, catalog } } });
      })
      .catch((error: unknown) => {
        if (!cancelled) setState({ key: workspaceId, result: { status: 'error', error } });
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceId, version]);
  const reload = useCallback(() => {
    setVersion((v) => v + 1);
  }, []);
  const result: Loadable<McpState> =
    state?.key === workspaceId ? state.result : { status: 'loading' };
  return { state: result, reload };
}
