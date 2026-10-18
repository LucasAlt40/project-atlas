import { useCallback, useEffect, useRef, useState } from 'react';
import { listSpecs, subscribeToSpecChanges } from '../services/specService';
import { subscribe } from '@/lib/subscribe';
import type { SpecSummary } from '../types';

export type SpecsState =
  | { status: 'loading' }
  | { status: 'error'; error: unknown }
  | { status: 'ready'; specs: SpecSummary[] };

/**
 * The specs of a workspace, read again whenever one changes (the core says so with an event: a
 * run ended, a question arrived, a decision was made). Nothing here guesses at progress.
 */
export function useSpecs(workspaceId: string) {
  const [state, setState] = useState<SpecsState>({ status: 'loading' });
  const alive = useRef(true);

  const reload = useCallback(() => {
    listSpecs(workspaceId)
      .then((specs) => {
        if (alive.current) setState({ status: 'ready', specs });
      })
      .catch((error: unknown) => {
        if (alive.current) {
          setState((current) =>
            current.status === 'ready' ? current : { status: 'error', error },
          );
        }
      });
  }, [workspaceId]);

  useEffect(() => {
    alive.current = true;
    reload();
    const unsubscribe = subscribe(() => subscribeToSpecChanges(reload));
    return () => {
      alive.current = false;
      unsubscribe();
    };
  }, [reload]);

  return { state, reload };
}
