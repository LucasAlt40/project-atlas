import { useEffect, useState } from 'react';
import { fetchAppInfo } from '../services/appInfoService';
import type { AppInfo } from '../types/appInfo';

export type AppInfoState =
  { status: 'loading' } | { status: 'ready'; info: AppInfo } | { status: 'error'; message: string };

export function useAppInfo(): AppInfoState {
  const [state, setState] = useState<AppInfoState>({ status: 'loading' });

  useEffect(() => {
    let cancelled = false;
    fetchAppInfo()
      .then((info) => {
        if (!cancelled) setState({ status: 'ready', info });
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setState({
            status: 'error',
            message: error instanceof Error ? error.message : String(error),
          });
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return state;
}
