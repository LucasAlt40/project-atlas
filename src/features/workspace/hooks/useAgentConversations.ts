import { useCallback, useEffect, useReducer, useState } from 'react';
import { conversationsReducer, initialConversations, runKey } from '../model/agentRuns';
import {
  listMessages,
  sendMessage,
  subscribeToExecutionEvents,
  subscribeToMessages,
} from '../services/workspaceService';
import type { ExecutionEvent, Message } from '../types';

/** Subscribes for the lifetime of the component; a late resolution is cleaned up. */
function useSubscription<T>(
  subscribe: (handler: (value: T) => void) => Promise<() => void>,
  handler: (value: T) => void,
) {
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    subscribe(handler)
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => {
        // Live updates are best-effort; conversations can always be re-read.
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [subscribe, handler]);
}

/**
 * Every agent's conversation and execution state in every workspace, fed by the core's events.
 * State is keyed by workspace and agent: there is no "current execution", so agents run
 * independently, and a run keeps being tracked while another workspace is on screen.
 */
export function useAgentConversations() {
  const [state, dispatch] = useReducer(conversationsReducer, initialConversations);
  // Changes when an assistant message arrives: by then the execution's usage is recorded.
  const [usageVersion, setUsageVersion] = useState(0);

  useEffect(() => {
    let cancelled = false;
    listMessages()
      .then((messages) => {
        if (!cancelled) dispatch({ type: 'loaded', messages });
      })
      .catch(() => {
        // Start with empty conversations if they cannot be read.
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const onEvent = useCallback((event: ExecutionEvent) => {
    dispatch({ type: 'event', event });
  }, []);
  const onMessage = useCallback((message: Message) => {
    dispatch({ type: 'message', message });
    if (message.role === 'assistant') setUsageVersion((v) => v + 1);
  }, []);
  useSubscription(subscribeToExecutionEvents, onEvent);
  useSubscription(subscribeToMessages, onMessage);

  const send = useCallback(async (workspaceId: string, agentId: string, content: string) => {
    try {
      const sent = await sendMessage({ workspaceId, agentId, content });
      dispatch({ type: 'sent', sent });
    } catch (error) {
      dispatch({ type: 'sendFailed', key: runKey(workspaceId, agentId), error });
    }
  }, []);

  return { state, send, usageVersion };
}
