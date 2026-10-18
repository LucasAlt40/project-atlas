import { useCallback, useEffect, useState } from 'react';
import { listReadySpecs, subscribeToSpecChanges } from '../services/specService';
import type { SpecSummary } from '../types';
import { subscribe } from '@/lib/subscribe';

/**
 * The specs a workflow could be started from (ready, not archived). A failed read is an empty list: the workflow screen works without specs.
 */
export function useReadySpecs(workspaceId: string | undefined) {
  const [specs, setSpecs] = useState<SpecSummary[]>([]);

  const reload = useCallback(() => {
    if (!workspaceId) return;
    listReadySpecs(workspaceId)
      .then(setSpecs)
      .catch(() => {
        setSpecs([]);
      });
  }, [workspaceId]);

  useEffect(() => {
    reload();
    return subscribe(() => subscribeToSpecChanges(reload));
  }, [reload]);

  return specs;
}
