import { useCallback, useEffect, useReducer, useState } from 'react';
import type {
  ExecutionRefDto,
  ExecutionWorktreeDto,
  SessionStatusEventDto,
  TerminalChunkDto,
} from '@/lib/tauri/commands';
import { conversationsReducer, initialConversations, runKey } from '../model/agentRuns';
import { TerminalHub } from '../model/terminalHub';
import {
  listExecutions,
  listExecutionWorktrees,
  listMessages,
  mergeExecution,
  sendMessage,
  subscribeToExecutionEvents,
  subscribeToMessages,
  subscribeToSessionStatus,
  subscribeToTerminalOutput,
} from '../services/workspaceService';
import type { ExecutionEvent, Message, StoredExecution } from '../types';

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
  // Terminal output is not React state: it can arrive many times a second.
  const [hub] = useState(() => new TerminalHub());
  // Executions that ended, kept by the core. Re-read when an answer arrives: by then the
  // execution that produced it has been stored.
  const [history, setHistory] = useState<StoredExecution[]>([]);
  // The Git worktree of each execution that ran isolated, by execution id. Re-read when an
  // answer arrives or a worktree event does: the core is the source of truth for it.
  const [worktrees, setWorktrees] = useState<Record<string, ExecutionWorktreeDto>>({});
  const [worktreeVersion, setWorktreeVersion] = useState(0);

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

  useEffect(() => {
    let cancelled = false;
    listExecutions()
      .then((executions) => {
        if (!cancelled) setHistory(executions);
      })
      .catch(() => {
        // The history is a convenience: the conversation still works without it.
      });
    return () => {
      cancelled = true;
    };
  }, [usageVersion]);

  useEffect(() => {
    let cancelled = false;
    listExecutionWorktrees()
      .then((list) => {
        if (!cancelled) setWorktrees(Object.fromEntries(list.map((w) => [w.executionId, w])));
      })
      .catch(() => {
        // Git details are extra: the execution is fine without them.
      });
    return () => {
      cancelled = true;
    };
  }, [usageVersion, worktreeVersion]);

  const onEvent = useCallback((event: ExecutionEvent) => {
    dispatch({ type: 'event', event });
    if (event.kind === 'worktree_created' || event.kind === 'worktree_finalized') {
      setWorktreeVersion((v) => v + 1);
    }
  }, []);
  const onMessage = useCallback((message: Message) => {
    dispatch({ type: 'message', message });
    if (message.role === 'assistant') setUsageVersion((v) => v + 1);
  }, []);
  useSubscription(subscribeToExecutionEvents, onEvent);
  useSubscription(subscribeToMessages, onMessage);
  const onOutput = useCallback(
    (chunk: TerminalChunkDto) => {
      hub.push(chunk);
    },
    [hub],
  );
  const onProcessStatus = useCallback((event: SessionStatusEventDto) => {
    dispatch({ type: 'processStatus', event });
  }, []);
  useSubscription(subscribeToTerminalOutput, onOutput);
  useSubscription(subscribeToSessionStatus, onProcessStatus);

  const send = useCallback(async (workspaceId: string, agentId: string, content: string) => {
    try {
      const sent = await sendMessage({ workspaceId, agentId, content });
      dispatch({ type: 'sent', sent });
    } catch (error) {
      dispatch({ type: 'sendFailed', key: runKey(workspaceId, agentId), error });
    }
  }, []);

  /** The user's explicit merge. Resolves with the worktree as it is afterwards. */
  const merge = useCallback(async (ref: ExecutionRefDto) => {
    const updated = await mergeExecution(ref);
    setWorktrees((current) => ({ ...current, [updated.executionId]: updated }));
    return updated;
  }, []);

  return { state, send, usageVersion, hub, history, worktrees, merge };
}
