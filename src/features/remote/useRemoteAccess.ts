import { useCallback, useEffect, useRef, useState } from 'react';
import type { RemoteStatusDto } from '@/lib/tauri/commands';
import { getRemoteStatus } from './remoteService';

export interface RemoteAccess {
  status: RemoteStatusDto | null;
  /** Shows what a command returned straight away, without waiting for the next read. */
  adopt: (status: RemoteStatusDto) => void;
}

/**
 * The state of mobile access. The core is the source of truth: it is read again every
 * `intervalMs`, which is how a phone pairing, a revoked device, an expired code or a changed
 * network address show up.
 */
export function useRemoteAccess(intervalMs: number): RemoteAccess {
  const [status, setStatus] = useState<RemoteStatusDto | null>(null);
  const alive = useRef(true);

  const read = useCallback(() => {
    getRemoteStatus()
      .then((next) => {
        if (alive.current) setStatus(next);
      })
      .catch(() => {
        // Outside the app (or while it starts) there is nothing to show; the next read tries again.
      });
  }, []);

  useEffect(() => {
    alive.current = true;
    read();
    const timer = setInterval(read, intervalMs);
    return () => {
      alive.current = false;
      clearInterval(timer);
    };
  }, [read, intervalMs]);

  return { status, adopt: setStatus };
}
