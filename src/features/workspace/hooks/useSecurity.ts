import { useCallback, useEffect, useState } from 'react';
import type { AgentPermissionsDto, WorkspaceSecurityDto } from '@/lib/tauri/commands';
import { getAgentPermissions, getWorkspaceSecurity } from '../services/workspaceService';

export type Loadable<T> =
  { status: 'loading' } | { status: 'error'; error: unknown } | { status: 'ready'; value: T };

/**
 * What the core says agents may do in the workspace. Read from the core every time the
 * workspace (or its folder) changes: the UI never decides or caches a policy of its own.
 */
export function useWorkspaceSecurity(
  workspaceId: string,
  projectPath: string,
): Loadable<WorkspaceSecurityDto> {
  const [state, setState] = useState<{ key: string; result: Loadable<WorkspaceSecurityDto> }>();
  const key = `${workspaceId}\u0000${projectPath}`;
  useEffect(() => {
    let cancelled = false;
    getWorkspaceSecurity(workspaceId)
      .then((value) => {
        if (!cancelled) setState({ key, result: { status: 'ready', value } });
      })
      .catch((error: unknown) => {
        if (!cancelled) setState({ key, result: { status: 'error', error } });
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceId, key]);
  // A result for another workspace is not an answer for this one.
  return state?.key === key ? state.result : { status: 'loading' };
}

/** One agent's permissions in a workspace, and a way to read them again after a change. */
export function useAgentPermissions(workspaceId: string, agentId: string) {
  const [state, setState] = useState<{ key: string; result: Loadable<AgentPermissionsDto> }>();
  const [version, setVersion] = useState(0);
  const key = `${workspaceId}/${agentId}`;
  useEffect(() => {
    let cancelled = false;
    getAgentPermissions(workspaceId, agentId)
      .then((value) => {
        if (!cancelled) setState({ key, result: { status: 'ready', value } });
      })
      .catch((error: unknown) => {
        if (!cancelled) setState({ key, result: { status: 'error', error } });
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceId, agentId, key, version]);
  const reload = useCallback(() => {
    setVersion((v) => v + 1);
  }, []);
  const result: Loadable<AgentPermissionsDto> =
    state?.key === key ? state.result : { status: 'loading' };
  return { permissions: result, reload };
}
