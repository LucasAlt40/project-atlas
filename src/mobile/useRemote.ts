import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { api, ApiError } from './api';
import type { ChangeEvent, RunDetail, RunSummary } from './types';

/**
 * How the page stands with the computer.
 * - `live`: the event stream is open;
 * - `reconnecting`: the stream dropped and is being reopened (what is shown may be stale);
 * - `offline`: the computer cannot be reached (it is off, asleep, or the Wi-Fi is gone).
 */
export type Link = 'connecting' | 'live' | 'reconnecting' | 'offline';

export type Phase = 'loading' | 'unpaired' | 'ready' | 'unreachable';

/** Why the page has no session, so it can say it. */
export type UnpairedReason = 'new' | 'revoked' | 'expired';

export type AlertKind = 'needs_answer' | 'needs_computer' | 'failed' | 'completed';

export interface Alert {
  id: string;
  kind: AlertKind;
  runId: string;
  title: string;
  at: number;
}

export interface Remote {
  phase: Phase;
  unpairedReason: UnpairedReason;
  deviceName: string | null;
  language: string | null;
  runs: RunSummary[];
  link: Link;
  /** When the runs were last read, on this clock. `null` before the first read. */
  lastSyncAt: number | null;
  /** The computer's clock minus this device's, to show durations right even if they differ. */
  clockOffset: number;
  alerts: Alert[];
  dismissAlert: (id: string) => void;
  refresh: () => Promise<void>;
  pair: (code: string, deviceName: string) => Promise<void>;
  /** Ends this phone's session on the computer. Rejects, and stays paired, if it could not. */
  signOut: () => Promise<void>;
  /** Tells `listener` about every change the stream announces and every reconnection. */
  onChange: (listener: (event: ChangeEvent | null) => void) => () => void;
  /** Called when a request is answered 401: the session is gone. */
  sessionLost: (reason: UnpairedReason) => void;
}

const POLL_MS = 30_000;
const REFRESH_DEBOUNCE_MS = 250;
const RECONNECT_MS = 3_000;
/** After the stream drops, how long before the page checks whether the computer answers at all. */
const PROBE_MS = 2_000;
const MAX_ALERTS = 4;

/** What changed between two readings that a person would want to be told about. */
export function alertsBetween(
  before: ReadonlyMap<string, RunSummary>,
  after: readonly RunSummary[],
  at: number,
): Alert[] {
  const found: Alert[] = [];
  for (const run of after) {
    const was = before.get(run.id);
    const title = run.workflowName;
    const alert = (kind: AlertKind): Alert => ({
      id: `${run.id}:${kind}:${String(at)}`,
      kind,
      runId: run.id,
      title,
      at,
    });
    if (
      run.attention === 'needs_answer' &&
      (was?.attention !== 'needs_answer' || run.pending > was.pending)
    ) {
      found.push(alert('needs_answer'));
    } else if (run.attention === 'needs_computer' && was?.attention !== 'needs_computer') {
      found.push(alert('needs_computer'));
    }
    if (was && was.status !== run.status) {
      if (run.status === 'failed') found.push(alert('failed'));
      else if (run.status === 'completed') found.push(alert('completed'));
    }
  }
  return found;
}

export function useRemote(): Remote {
  const [phase, setPhase] = useState<Phase>('loading');
  const [unpairedReason, setUnpairedReason] = useState<UnpairedReason>('new');
  const [deviceName, setDeviceName] = useState<string | null>(null);
  const [language, setLanguage] = useState<string | null>(null);
  const [runs, setRuns] = useState<RunSummary[]>([]);
  const [link, setLink] = useState<Link>('connecting');
  const [lastSyncAt, setLastSyncAt] = useState<number | null>(null);
  const [clockOffset, setClockOffset] = useState(0);
  const [alerts, setAlerts] = useState<Alert[]>([]);

  const known = useRef<Map<string, RunSummary> | null>(null);
  const listeners = useRef(new Set<(event: ChangeEvent | null) => void>());
  const refreshing = useRef<Promise<void> | null>(null);

  const sessionLost = useCallback((reason: UnpairedReason) => {
    known.current = null;
    setRuns([]);
    setAlerts([]);
    setDeviceName(null);
    setUnpairedReason(reason);
    setPhase('unpaired');
  }, []);

  const refresh = useCallback((): Promise<void> => {
    // Reads that overlap share one request: an event storm costs one round trip.
    if (refreshing.current) return refreshing.current;
    const work = (async () => {
      try {
        const overview = await api.runs();
        const now = Date.now();
        setClockOffset(overview.serverTime - now);
        setLastSyncAt(now);
        setRuns(overview.runs);
        setLanguage(overview.language);
        if (known.current) {
          const fresh = alertsBetween(known.current, overview.runs, now);
          if (fresh.length > 0) {
            setAlerts((current) => [...fresh, ...current].slice(0, MAX_ALERTS));
            if (fresh.some((a) => a.kind === 'needs_answer')) {
              try {
                navigator.vibrate(120);
              } catch {
                // Not every browser lets a page vibrate, and some only after a tap.
              }
            }
          }
        }
        known.current = new Map(overview.runs.map((run) => [run.id, run]));
        setLink((current) => (current === 'offline' ? 'reconnecting' : current));
      } catch (error) {
        if (error instanceof ApiError && error.unauthenticated) {
          sessionLost(error.code === 'session_expired' ? 'expired' : 'revoked');
        } else {
          setLink('offline');
        }
      } finally {
        refreshing.current = null;
      }
    })();
    refreshing.current = work;
    return work;
  }, [sessionLost]);

  // Is there a session already? (The page was paired before and is being opened again.)
  useEffect(() => {
    let cancelled = false;
    api
      .session()
      .then((session) => {
        if (cancelled) return;
        setDeviceName(session.device.name);
        if (session.language) setLanguage(session.language);
        setPhase('ready');
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        if (error instanceof ApiError && error.unauthenticated) {
          setUnpairedReason(error.code === 'session_expired' ? 'expired' : 'new');
          setPhase('unpaired');
        } else {
          setPhase('unreachable');
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const notify = useCallback((event: ChangeEvent | null) => {
    for (const listener of listeners.current) listener(event);
  }, []);

  // While paired: read the runs, then follow the stream. The stream only says that something
  // changed; what changed is read again, so a lost or repeated event costs one more read.
  useEffect(() => {
    if (phase !== 'ready') return undefined;
    let closed = false;
    let source: EventSource | null = null;
    let debounce: ReturnType<typeof setTimeout> | undefined;
    let reconnect: ReturnType<typeof setTimeout> | undefined;
    let probe: ReturnType<typeof setTimeout> | undefined;

    const changed = (event: ChangeEvent | null) => {
      clearTimeout(debounce);
      debounce = setTimeout(() => {
        void refresh();
        notify(event);
      }, REFRESH_DEBOUNCE_MS);
    };

    const open = () => {
      if (closed) return;
      source = new EventSource('/api/events');
      source.addEventListener('open', () => {
        setLink('live');
      });
      source.addEventListener('resync', () => {
        setLink('live');
        changed(null);
      });
      source.addEventListener('change', (message) => {
        try {
          changed(JSON.parse((message as MessageEvent<string>).data) as ChangeEvent);
        } catch {
          changed(null);
        }
      });
      source.addEventListener('revoked', () => {
        source?.close();
        sessionLost('revoked');
      });
      source.addEventListener('error', () => {
        setLink('reconnecting');
        // The browser keeps retrying by itself; the page asks too, so that a computer that does
        // not answer is called unreachable rather than left "reconnecting" for good.
        clearTimeout(probe);
        probe = setTimeout(() => {
          void refresh();
        }, PROBE_MS);
        // The browser retries on its own while the connection merely dropped; a stream the
        // server refused is closed for good, and the page finds out why itself.
        if (source?.readyState === EventSource.CLOSED) {
          source.close();
          reconnect = setTimeout(() => {
            api
              .session()
              .then(open)
              .catch((error: unknown) => {
                if (error instanceof ApiError && error.unauthenticated) {
                  sessionLost(error.code === 'session_expired' ? 'expired' : 'revoked');
                } else {
                  setLink('offline');
                  reconnect = setTimeout(open, RECONNECT_MS);
                }
              });
          }, RECONNECT_MS);
        }
      });
    };

    void refresh();
    open();
    const poll = setInterval(() => {
      if (document.visibilityState === 'visible') void refresh();
    }, POLL_MS);
    const onVisible = () => {
      if (document.visibilityState === 'visible') {
        void refresh();
        notify(null);
      }
    };
    document.addEventListener('visibilitychange', onVisible);
    window.addEventListener('online', onVisible);
    return () => {
      closed = true;
      clearTimeout(debounce);
      clearTimeout(reconnect);
      clearTimeout(probe);
      clearInterval(poll);
      source?.close();
      document.removeEventListener('visibilitychange', onVisible);
      window.removeEventListener('online', onVisible);
    };
  }, [phase, refresh, notify, sessionLost]);

  const pair = useCallback(async (code: string, name: string) => {
    const session = await api.pair(code, name);
    known.current = null;
    setDeviceName(session.device.name);
    setPhase('ready');
  }, []);

  const signOut = useCallback(async () => {
    try {
      await api.logout();
    } catch (failure) {
      // A session that is already gone ends here. Any other failure leaves it valid on the
      // computer (and the cookie in this browser): saying "disconnected" would not be true.
      if (!(failure instanceof ApiError && failure.unauthenticated)) throw failure;
    }
    sessionLost('new');
  }, [sessionLost]);

  const onChange = useCallback((listener: (event: ChangeEvent | null) => void) => {
    listeners.current.add(listener);
    return () => {
      listeners.current.delete(listener);
    };
  }, []);

  const dismissAlert = useCallback((id: string) => {
    setAlerts((current) => current.filter((alert) => alert.id !== id));
  }, []);

  return useMemo(
    () => ({
      phase,
      unpairedReason,
      deviceName,
      language,
      runs,
      link,
      lastSyncAt,
      clockOffset,
      alerts,
      dismissAlert,
      refresh,
      pair,
      signOut,
      onChange,
      sessionLost,
    }),
    [
      phase,
      unpairedReason,
      deviceName,
      language,
      runs,
      link,
      lastSyncAt,
      clockOffset,
      alerts,
      dismissAlert,
      refresh,
      pair,
      signOut,
      onChange,
      sessionLost,
    ],
  );
}

export interface RunDetailState {
  run: RunDetail | null;
  /** `null`: loaded; `'not_found'`: the run is gone; `'failed'`: could not be read now. */
  problem: 'not_found' | 'failed' | null;
  reload: () => Promise<void>;
  /** Shows what the backend confirmed for an action, without waiting for the next read. */
  adopt: (run: RunDetail) => void;
}

/** One run, read again whenever the stream says something about it changed. */
export function useRunDetail(remote: Remote, id: string): RunDetailState {
  const [run, setRun] = useState<RunDetail | null>(null);
  const [problem, setProblem] = useState<RunDetailState['problem']>(null);
  const { onChange, sessionLost } = remote;
  // The newest request wins: an older answer must never overwrite a newer one.
  const sequence = useRef(0);

  const reload = useCallback((): Promise<void> => {
    const mine = ++sequence.current;
    return api
      .run(id)
      .then((detail) => {
        if (mine !== sequence.current) return;
        setRun(detail);
        setProblem(null);
      })
      .catch((error: unknown) => {
        if (mine !== sequence.current) return;
        if (error instanceof ApiError && error.unauthenticated) {
          sessionLost(error.code === 'session_expired' ? 'expired' : 'revoked');
        } else if (error instanceof ApiError && error.status === 404) {
          setProblem('not_found');
        } else if (!(error instanceof ApiError && error.unreachable)) {
          setProblem('failed');
        }
        // Unreachable: keep what is shown; the page says the computer cannot be reached.
      });
  }, [id, sessionLost]);

  useEffect(() => {
    // A different run is a different screen (the page keys it by id), so nothing is reset here.
    void reload();
    return onChange((event) => {
      if (!event || event.runId === id || event.runId === null) void reload();
    });
  }, [id, onChange, reload]);

  const adopt = useCallback((detail: RunDetail) => {
    sequence.current += 1;
    setRun(detail);
    setProblem(null);
  }, []);

  return { run, problem, reload, adopt };
}
