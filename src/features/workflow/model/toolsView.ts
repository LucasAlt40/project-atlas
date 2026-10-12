import type {
  McpConnectionDto,
  McpGrantDto,
  McpServerStatusDto,
  McpToolSelectionDto,
} from '@/lib/tauri/commands';
import type { Workflow } from '../types';

/** What the screen can truthfully say about one connection in this workflow. */
export interface ConnectionView {
  connection: McpConnectionDto;
  /** The last look at the server, `null` when it was never looked at. */
  status: McpServerStatusDto | null;
  /** The steps of this workflow it is granted to (by step), and which tools. */
  steps: { nodeId: string; label: string; tools: McpToolSelectionDto }[];
  /** Granted to the whole workflow or to an agent, not to a step: it applies wherever that matches. */
  broad: number;
}

/** Joins the workspace's connections with the grants that name this workflow or its steps. */
export function connectionViews(
  workflow: Workflow,
  connections: readonly McpConnectionDto[],
  grants: readonly McpGrantDto[],
): ConnectionView[] {
  const labelOf = (id: string) => workflow.nodes.find((n) => n.id === id)?.label ?? id;
  return connections.map((connection) => {
    const mine = grants.filter((g) => g.connectionId === connection.id);
    return {
      connection,
      status: connection.discovery?.status ?? null,
      steps: mine
        .filter((g) => g.workflowId === workflow.id && g.nodeId !== null)
        .map((g) => ({ nodeId: g.nodeId ?? '', label: labelOf(g.nodeId ?? ''), tools: g.tools })),
      broad: mine.filter(
        (g) => g.nodeId === null && (g.workflowId === workflow.id || g.workflowId === null),
      ).length,
    };
  });
}
