import { useCallback, useEffect, useRef, useState } from 'react';
import { listPendingInteractions, subscribeToWorkflowEvents } from '../services/workflowService';
import type { PendingInteraction } from '../types';

/**
 * The questions agents are waiting on in a workspace. The core is the source of truth (it keeps
 * them with the run, so they survive navigation and a restart); a workflow event only says that
 * something changed, and the list is read again.
 */
export function usePendingInteractions(workspaceId: string | undefined): PendingInteraction[] {
  const [loaded, setLoaded] = useState<{ workspaceId: string; items: PendingInteraction[] }>();
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const refresh = useCallback(() => {
    timer.current = undefined;
    if (!workspaceId) return;
    listPendingInteractions(workspaceId)
      .then((items) => {
        setLoaded({ workspaceId, items });
      })
      .catch(() => {
        // A missed update is repaired by the next event.
      });
  }, [workspaceId]);

  useEffect(() => {
    refresh();
    let stop: (() => void) | undefined;
    let disposed = false;
    subscribeToWorkflowEvents((event) => {
      if (event.workspaceId !== workspaceId) return;
      timer.current ??= setTimeout(refresh, 40);
    })
      .then((unlisten) => {
        if (disposed) unlisten();
        else stop = unlisten;
      })
      .catch(() => {
        // Live updates are best-effort.
      });
    return () => {
      disposed = true;
      stop?.();
      if (timer.current) clearTimeout(timer.current);
      timer.current = undefined;
    };
  }, [workspaceId, refresh]);

  return loaded && loaded.workspaceId === workspaceId ? loaded.items : [];
}
