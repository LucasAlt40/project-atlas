import { useCallback, useEffect, useState } from 'react';
import type { HarnessSummaryDto } from '@/lib/tauri/commands';
import { getProjectHarness } from '@/features/workspace/services/workspaceService';

export type HarnessState =
  | { status: 'loading' }
  | { status: 'error'; error: unknown }
  | { status: 'ready'; summary: HarnessSummaryDto };

/**
 * The Harness state of a workspace's project. It is read from the project (`.atlas/`) each time
 * the workspace or its folder changes, never cached across workspaces.
 */
export function useProjectHarness(workspaceId: string, projectPath = '') {
  const key = `${workspaceId}\u0000${projectPath}`;
  const [loaded, setLoaded] = useState<{ key: string; state: HarnessState } | null>(null);
  const [version, setVersion] = useState(0);

  useEffect(() => {
    let cancelled = false;
    getProjectHarness(workspaceId)
      .then((summary) => {
        if (!cancelled) setLoaded({ key, state: { status: 'ready', summary } });
      })
      .catch((error: unknown) => {
        if (!cancelled) setLoaded({ key, state: { status: 'error', error } });
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceId, key, version]);

  const reload = useCallback(() => {
    setVersion((v) => v + 1);
  }, []);
  const replace = useCallback(
    (summary: HarnessSummaryDto) => {
      setLoaded({ key, state: { status: 'ready', summary } });
    },
    [key],
  );

  // Anything loaded for another workspace or folder is "still loading".
  const state: HarnessState = loaded?.key === key ? loaded.state : { status: 'loading' };
  return { state, reload, replace };
}
