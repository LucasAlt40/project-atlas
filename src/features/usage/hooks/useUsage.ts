import { useEffect, useState } from 'react';
import type { AgentUsageSummaryDto, WorkspaceUsageSummaryDto } from '@/lib/tauri/commands';
import { getAgentUsage, getWorkspaceUsage } from '../services/usageService';

export type UsageState<T> =
  { status: 'loading' } | { status: 'error'; error: unknown } | { status: 'ready'; summary: T };

function useSummary<T>(load: () => Promise<T>, dependencies: readonly unknown[]): UsageState<T> {
  const [state, setState] = useState<UsageState<T>>({ status: 'loading' });
  useEffect(() => {
    let cancelled = false;
    load()
      .then((summary) => {
        if (!cancelled) setState({ status: 'ready', summary });
      })
      .catch((error: unknown) => {
        if (!cancelled) setState({ status: 'error', error });
      });
    return () => {
      cancelled = true;
    };
    // `load` is rebuilt every render; the dependencies say when to fetch again.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, dependencies);
  return state;
}

/**
 * One agent's usage in a workspace. `version` changes whenever an execution finishes (see
 * `WorkspaceProvider`), which is when the numbers can have changed.
 */
export function useAgentUsage(
  workspaceId: string,
  agentId: string,
  version: number,
): UsageState<AgentUsageSummaryDto> {
  return useSummary(() => getAgentUsage(workspaceId, agentId), [workspaceId, agentId, version]);
}

export function useWorkspaceUsage(
  workspaceId: string,
  version: number,
): UsageState<WorkspaceUsageSummaryDto> {
  return useSummary(() => getWorkspaceUsage(workspaceId), [workspaceId, version]);
}
