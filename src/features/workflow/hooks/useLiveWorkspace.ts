import { useCallback, useEffect, useRef, useState } from 'react';
import type { LiveWorkspaceStateDto, LiveWorkspaceUpdateDto } from '@/lib/tauri/commands';
import {
  activityOf,
  applyUpdate,
  modelOf,
  pushActivity,
  type ActivityEntry,
  type LiveModel,
} from '../model/live';
import {
  getLiveWorkspace,
  refreshLiveWorkspace,
  subscribeToLiveUpdates,
} from '../services/liveWorkspaceService';

export type LiveLoad =
  | { status: 'loading' }
  | { status: 'error'; error: unknown }
  /** `model` is `null` when the run has no code worktree. */
  | { status: 'ready'; model: LiveModel | null };

export interface LiveWorkspace {
  load: LiveLoad;
  activity: ActivityEntry[];
  /** Compares the whole worktree with the baseline now. */
  refresh: () => void;
  refreshing: boolean;
}

/**
 * Follows the Live Workspace of one run: the backend's snapshot, then its updates.
 *
 * - The listener is set up *before* the snapshot is asked for, and updates that arrive meanwhile
 *   wait for it, so none is lost between the two.
 * - Only the next revision is applied. An older one is dropped; a gap makes the snapshot be read
 *   again (and the updates that arrive during that wait their turn).
 * - It follows one run: a screen for another run mounts its own (the panel is keyed by the run).
 * - There is no timer here: the backend watches the worktree and says when something changed.
 */
export function useLiveWorkspace(runId: string | null): LiveWorkspace {
  const [load, setLoad] = useState<LiveLoad>({ status: 'loading' });
  const [activity, setActivity] = useState<ActivityEntry[]>([]);
  const [refreshing, setRefreshing] = useState(false);
  const modelRef = useRef<LiveModel | null>(null);
  const resync = useRef<(() => void) | null>(null);
  const refreshRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    modelRef.current = null;
    if (runId === null) return;
    let cancelled = false;
    let syncing = true;
    let queue: LiveWorkspaceUpdateDto[] = [];

    const show = (model: LiveModel | null) => {
      modelRef.current = model;
      setLoad({ status: 'ready', model });
    };
    const apply = (update: LiveWorkspaceUpdateDto) => {
      const current = modelRef.current;
      if (current === null) return;
      const result = applyUpdate(current, update);
      if (result.kind === 'stale') return;
      if (result.kind === 'gap') {
        read();
        return;
      }
      const entries = activityOf(current, update, result.model);
      show(result.model);
      if (entries.length > 0) setActivity((list) => pushActivity(list, entries));
    };
    const settle = (state: LiveWorkspaceStateDto | null) => {
      show(state === null ? null : modelOf(state));
      syncing = false;
      const waiting = queue;
      queue = [];
      for (const update of waiting) apply(update);
    };
    const read = () => {
      syncing = true;
      getLiveWorkspace(runId)
        .then((state) => {
          if (!cancelled) settle(state);
        })
        .catch((error: unknown) => {
          if (cancelled) return;
          syncing = false;
          queue = [];
          setLoad({ status: 'error', error });
        });
    };
    resync.current = read;
    refreshRef.current = () => {
      setRefreshing(true);
      refreshLiveWorkspace(runId)
        .then((state) => {
          if (!cancelled) settle(state);
        })
        .catch((error: unknown) => {
          if (!cancelled) setLoad({ status: 'error', error });
        })
        .finally(() => {
          if (!cancelled) setRefreshing(false);
        });
    };

    let stop: (() => void) | null = null;
    void subscribeToLiveUpdates((update) => {
      if (cancelled || update.runId !== runId) return;
      if (syncing) queue.push(update);
      else apply(update);
    }).then((unlisten) => {
      if (cancelled) unlisten();
      else stop = unlisten;
    });
    read();

    return () => {
      cancelled = true;
      resync.current = null;
      refreshRef.current = null;
      stop?.();
    };
  }, [runId]);

  const refresh = useCallback(() => {
    refreshRef.current?.();
  }, []);

  return { load, activity, refresh, refreshing };
}
