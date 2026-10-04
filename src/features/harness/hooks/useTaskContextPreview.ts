import { useEffect, useState } from 'react';
import type { TaskContextPreviewDto } from '@/lib/tauri/commands';
import { previewTaskContext } from '@/features/workspace/services/workspaceService';

export type TaskContextPreviewState =
  | { status: 'idle' }
  | { status: 'loading' }
  | { status: 'error' }
  | { status: 'ready'; preview: TaskContextPreviewDto };

/** How long the user pauses typing before the context is chosen again. */
const DEBOUNCE_MS = 350;

/**
 * What the agent would be told for the task being typed. Asked after a pause, once per pause;
 * an answer for text that has since changed is dropped. Inspection only: nothing is run.
 * Failing to preview never gets in the way of sending.
 */
export function useTaskContextPreview(
  workspaceId: string,
  agentId: string,
  task: string,
): TaskContextPreviewState {
  const text = task.trim();
  const key = `${workspaceId}\u0000${agentId}\u0000${text}`;
  const [loaded, setLoaded] = useState<{ key: string; state: TaskContextPreviewState } | null>(
    null,
  );

  useEffect(() => {
    if (text === '') return;
    let cancelled = false;
    const timer = setTimeout(() => {
      void Promise.resolve(previewTaskContext(workspaceId, agentId, text))
        .then((preview: TaskContextPreviewDto | undefined) => {
          if (cancelled) return;
          setLoaded({
            key,
            state: preview ? { status: 'ready', preview } : { status: 'idle' },
          });
        })
        .catch(() => {
          if (!cancelled) setLoaded({ key, state: { status: 'error' } });
        });
    }, DEBOUNCE_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [workspaceId, agentId, text, key]);

  if (text === '') return { status: 'idle' };
  return loaded?.key === key ? loaded.state : { status: 'loading' };
}
