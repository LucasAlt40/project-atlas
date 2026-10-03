import { useCallback, useState } from 'react';
import type { ExecutionRefDto } from '@/lib/tauri/commands';
import { interruptExecution, terminateExecution } from '../services/workspaceService';

/**
 * Interrupt and terminate for one execution's process. The core decides whether the request is
 * valid (it names the session by all three ids); this only reports what it answered.
 */
export function useProcessControls(ref: ExecutionRefDto | null) {
  const [error, setError] = useState<unknown>(undefined);
  const executionId = ref?.executionId;
  const workspaceId = ref?.workspaceId;
  const agentId = ref?.agentId;

  const call = useCallback(
    async (action: (ref: ExecutionRefDto) => Promise<void>) => {
      if (!executionId || !workspaceId || !agentId) return;
      setError(undefined);
      try {
        await action({ executionId, workspaceId, agentId });
      } catch (failure) {
        setError(failure);
      }
    },
    [executionId, workspaceId, agentId],
  );

  const interrupt = useCallback(() => call(interruptExecution), [call]);
  const terminate = useCallback(() => call(terminateExecution), [call]);
  return { interrupt, terminate, error };
}
