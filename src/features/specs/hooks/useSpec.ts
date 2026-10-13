import { useCallback, useEffect, useRef, useState } from 'react';
import { getSpec, subscribeToSpecChanges } from '../services/specService';
import { subscribe } from './subscribe';
import type { SpecAction, SpecView } from '../types';

export type SpecState =
  { status: 'loading' } | { status: 'error'; error: unknown } | { status: 'ready'; view: SpecView };

/** What an action gave back: a new view, or a view and what could not be started. */
export type Outcome = SpecView | SpecAction;

function isAction(outcome: Outcome): outcome is SpecAction {
  return 'view' in outcome;
}

/**
 * One spec, as the core says it is. The screen shows exactly this: it follows the spec's own
 * change events, and never moves to an older version than the one it already has.
 *
 * `act` runs one thing the person asked for. Only one runs at a time (a second click while the
 * first is on its way does nothing), and what went wrong is kept for the screen to say.
 */
export function useSpec(specId: string) {
  const [state, setState] = useState<SpecState>({ status: 'loading' });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const [launchError, setLaunchError] = useState<unknown>(null);
  const working = useRef(false);
  const alive = useRef(true);

  const adopt = useCallback((view: SpecView) => {
    if (!alive.current) return;
    // A read that was already on its way when something newer arrived must not undo it.
    setState((current) =>
      current.status === 'ready' && current.view.spec.version > view.spec.version
        ? current
        : { status: 'ready', view },
    );
  }, []);

  const reload = useCallback(() => {
    getSpec(specId)
      .then(adopt)
      .catch((failure: unknown) => {
        if (alive.current) {
          setState((current) =>
            current.status === 'ready' ? current : { status: 'error', error: failure },
          );
        }
      });
  }, [specId, adopt]);

  useEffect(() => {
    alive.current = true;
    reload();
    const unsubscribe = subscribe(() =>
      subscribeToSpecChanges((change) => {
        if (change.specId === specId) reload();
      }),
    );
    return () => {
      alive.current = false;
      unsubscribe();
    };
  }, [specId, reload]);

  const act = useCallback(
    async (run: () => Promise<Outcome>): Promise<boolean> => {
      if (working.current) return false;
      working.current = true;
      setBusy(true);
      setError(null);
      setLaunchError(null);
      try {
        const outcome = await run();
        if (isAction(outcome)) {
          adopt(outcome.view);
          if (outcome.launchError) setLaunchError(outcome.launchError);
        } else {
          adopt(outcome);
        }
        return true;
      } catch (failure: unknown) {
        if (alive.current) setError(failure);
        // Whatever was refused, the screen shows what the core now holds.
        reload();
        return false;
      } finally {
        working.current = false;
        if (alive.current) setBusy(false);
      }
    },
    [adopt, reload],
  );

  const dismiss = useCallback(() => {
    setError(null);
    setLaunchError(null);
  }, []);

  return { state, busy, error, launchError, act, reload, dismiss };
}
