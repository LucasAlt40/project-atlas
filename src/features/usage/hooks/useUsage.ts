import { useEffect, useEffectEvent, useState } from 'react';
import type { AgentUsageSummaryDto, WorkspaceUsageSummaryDto } from '@/lib/tauri/commands';
import { getAgentUsage, getWorkspaceUsage } from '../services/usageService';

export type UsageState<T> =
  { status: 'loading' } | { status: 'error'; error: unknown } | { status: 'ready'; summary: T };

/** Fetches with `load` whenever `key` changes (`load` itself is rebuilt every render). */
function useSummary<T>(load: () => Promise<T>, key: string): UsageState<T> {
  const [state, setState] = useState<UsageState<T>>({ status: 'loading' });
  const fetchSummary = useEffectEvent(load);
  useEffect(() => {
    let cancelled = false;
    fetchSummary()
      .then((summary) => {
        if (!cancelled) setState({ status: 'ready', summary });
      })
      .catch((error: unknown) => {
        if (!cancelled) setState({ status: 'error', error });
      });
    return () => {
      cancelled = true;
    };
  }, [key]);
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
  return useSummary(
    () => getAgentUsage(workspaceId, agentId),
    JSON.stringify([workspaceId, agentId, version]),
  );
}

export function useWorkspaceUsage(
  workspaceId: string,
  version: number,
): UsageState<WorkspaceUsageSummaryDto> {
  return useSummary(() => getWorkspaceUsage(workspaceId), JSON.stringify([workspaceId, version]));
}
