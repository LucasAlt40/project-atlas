import { useCallback, useEffect, useState } from 'react';
import type { ProjectInfoDto, WorkspaceHarnessOverviewDto } from '@/lib/tauri/commands';
import {
  getWorkspaceHarness,
  listWorkspaceProjects,
  subscribeToWorkspaceHarnessProgress,
} from '../services/workspaceService';

export type ProjectsState =
  | { status: 'loading' }
  | { status: 'error'; error: unknown }
  | {
      status: 'ready';
      projects: ProjectInfoDto[];
      overview: WorkspaceHarnessOverviewDto;
    };

/**
 * The projects of a workspace, the state of each project's Harness and whether the workspace
 * context is current: read from the folders each time the projects change or the window is
 * focused again. `running` holds the projects whose Harness is being started right now.
 *
 * `signature` changes when the registered projects do (an add, a removal, a new path), so what
 * was read for the old ones is never shown for the new ones.
 */
export function useProjectsOverview(workspaceId: string, signature: string) {
  const key = `${workspaceId}\u0000${signature}`;
  const [loaded, setLoaded] = useState<{ key: string; state: ProjectsState } | null>(null);
  const [version, setVersion] = useState(0);
  const [running, setRunning] = useState<Record<string, true>>({});

  useEffect(() => {
    let cancelled = false;
    Promise.all([listWorkspaceProjects(workspaceId), getWorkspaceHarness(workspaceId)])
      .then(([projects, overview]) => {
        if (!cancelled) setLoaded({ key, state: { status: 'ready', projects, overview } });
      })
      .catch((error: unknown) => {
        if (!cancelled) setLoaded({ key, state: { status: 'error', error } });
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceId, key, version]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void subscribeToWorkspaceHarnessProgress((update) => {
      if (update.workspaceId !== workspaceId) return;
      setRunning((current) => {
        const others = Object.fromEntries(
          Object.entries(current).filter(([id]) => id !== update.projectId),
        );
        return update.state === 'running' ? { ...others, [update.projectId]: true } : others;
      });
    }).then((off) => {
      if (disposed) off();
      else unlisten = off;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [workspaceId]);

  // The folders may have changed while Atlas was in the background: look again on return.
  useEffect(() => {
    const onFocus = () => {
      setVersion((v) => v + 1);
    };
    window.addEventListener('focus', onFocus);
    return () => {
      window.removeEventListener('focus', onFocus);
    };
  }, []);

  const reload = useCallback(() => {
    setVersion((v) => v + 1);
  }, []);

  const state: ProjectsState = loaded?.key === key ? loaded.state : { status: 'loading' };
  return { state, running, reload };
}
