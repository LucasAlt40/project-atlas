import { isTerminal, toActivityEntry, type ActivityEntry } from './activity';
import { applyApprovalEvent, mergeApprovals, type ApprovalRequest } from './approvals';
import type { SessionStatusEventDto } from '@/lib/tauri/commands';
import type { ExecutionEvent, Message, SentMessage } from '../types';

/** Conversations and runs belong to an agent *in a workspace*. */
export function runKey(workspaceId: string, agentId: string): string {
  return `${workspaceId}/${agentId}`;
}

/**
 * What the app knows about one agent's current (or latest) execution in one workspace.
 * Everything is keyed by workspace + agent, so agents running at the same time (even the same
 * agent in two workspaces) never touch each other's state.
 */
export interface AgentRun {
  executionId: string;
  status: 'running' | 'completed' | 'failed' | 'cancelled';
  /** Milliseconds since the Unix epoch when the run started. */
  startedAt: number;
  activity: ActivityEntry[];
  /** Set when the run failed; lets the UI tell "authentication required" from other failures. */
  failureKind: string | null;
  /** Answer text is arriving right now (cleared when a tool call or another step starts). */
  receiving: boolean;
  /** The core asked the user to approve something and has not been answered yet. */
  pendingApprovalId: string | null;
}

/** The live answer of an execution that is still running. */
export interface LiveResponse {
  executionId: string;
  text: string;
}

/**
 * The process behind an execution, as the core last reported it. Kept apart from the run: its
 * output lives in the `TerminalHub`, its controls in the terminal view.
 */
export interface ProcessState {
  processSessionId: string;
  status: SessionStatusEventDto['status'];
  userAction: SessionStatusEventDto['userAction'];
  exitCode: number | null;
  /** When the process was first seen running (milliseconds since the Unix epoch). */
  startedAt: number;
  /** When it exited, once it has. */
  endedAt: number | null;
}

export interface ConversationsState {
  messages: Record<string, Message[]>;
  runs: Record<string, AgentRun>;
  /** Text streamed so far for each running execution; replaced by its final message. */
  streams: Record<string, LiveResponse>;
  /** A rejected send (e.g. the agent is busy). */
  sendErrors: Record<string, unknown>;
  /** Commands waiting for the user's decision, in every workspace. */
  approvals: ApprovalRequest[];
  /** The process of each execution that has run in a terminal, by execution id. */
  processes: Record<string, ProcessState>;
}

export const initialConversations: ConversationsState = {
  messages: {},
  runs: {},
  streams: {},
  sendErrors: {},
  approvals: [],
  processes: {},
};

export type ConversationAction =
  | { type: 'loaded'; messages: Message[] }
  | { type: 'sent'; sent: SentMessage }
  | { type: 'sendFailed'; key: string; error: unknown }
  | { type: 'event'; event: ExecutionEvent }
  | { type: 'message'; message: Message }
  | { type: 'approvalsLoaded'; approvals: ApprovalRequest[] }
  | { type: 'approvalGone'; id: string }
  | { type: 'processStatus'; event: SessionStatusEventDto };

function without<T>(record: Record<string, T>, key: string): Record<string, T> {
  return Object.fromEntries(Object.entries(record).filter(([k]) => k !== key));
}

function addMessage(messages: Message[], message: Message): Message[] {
  if (messages.some((m) => m.id === message.id)) return messages;
  return [...messages, message].sort((a, b) => a.timestamp - b.timestamp);
}

function failureKindOf(event: ExecutionEvent): string | null {
  return event.kind === 'failed' ? (event.metadata.failureKind ?? null) : null;
}

/** The approval the execution is waiting on after `event`, if any. */
function pendingAfter(current: string | null, event: ExecutionEvent): string | null {
  if (isTerminal(event.kind)) return null;
  if (event.kind !== 'permission') return current;
  const id = event.metadata.approvalId ?? null;
  switch (event.metadata.decision) {
    case 'approval_requested':
      return id;
    case 'approved':
    case 'rejected':
      return id === current ? null : current;
    default:
      return current;
  }
}

function statusAfter(current: AgentRun['status'], event: ExecutionEvent): AgentRun['status'] {
  // The first ending is the ending: nothing that arrives later changes how it ended.
  if (current !== 'running') return current;
  switch (event.kind) {
    case 'completed':
      return 'completed';
    case 'failed':
      return 'failed';
    case 'cancelled':
      return 'cancelled';
    default:
      return current;
  }
}

function applyEvent(run: AgentRun | undefined, event: ExecutionEvent): AgentRun | undefined {
  const sameExecution = run?.executionId === event.executionId;
  if (event.kind === 'output_chunk') {
    // Streamed text only means something inside the execution it belongs to, and only until
    // that execution has ended: a late piece never reopens it.
    if (!run || !sameExecution || run.status !== 'running') return run;
    if (run.receiving) return run;
    // The first piece of a stretch of text: note it once instead of once per token.
    const entry = toActivityEntry(event, run.activity.length);
    return { ...run, receiving: true, activity: [...run.activity, { ...entry, metadata: {} }] };
  }
  if (run && sameExecution) {
    // The same execution: append, and move to its final state on a terminal event.
    return {
      ...run,
      status: statusAfter(run.status, event),
      failureKind:
        event.kind === 'failed' && run.status === 'running'
          ? failureKindOf(event)
          : run.failureKind,
      receiving: false,
      pendingApprovalId: pendingAfter(run.pendingApprovalId, event),
      activity: [...run.activity, toActivityEntry(event, run.activity.length)],
    };
  }
  if (run?.status === 'running') {
    // A late event of an older execution while a newer one runs: not ours any more.
    return run;
  }
  // The agent was idle or finished. Once it has shown an execution, only the start of another
  // one (or a failure that has no start, such as a rejected request) begins a run: any other
  // event of an execution the agent no longer shows is a late one and must not bring it back.
  // With nothing shown yet (the app was just opened) a run already under way is picked up.
  if (run && event.kind !== 'started' && !isTerminal(event.kind)) return run;
  return {
    executionId: event.executionId,
    status: statusAfter('running', event),
    startedAt: event.timestamp,
    failureKind: failureKindOf(event),
    receiving: false,
    pendingApprovalId: pendingAfter(null, event),
    activity: [toActivityEntry(event, 0)],
  };
}

function appendToStream(
  streams: Record<string, LiveResponse>,
  key: string,
  event: ExecutionEvent,
): Record<string, LiveResponse> {
  const current = streams[key];
  const text = current?.executionId === event.executionId ? current.text : '';
  return { ...streams, [key]: { executionId: event.executionId, text: text + event.message } };
}

export function conversationsReducer(
  state: ConversationsState,
  action: ConversationAction,
): ConversationsState {
  switch (action.type) {
    case 'loaded': {
      const messages: Record<string, Message[]> = { ...state.messages };
      for (const message of action.messages) {
        const key = runKey(message.workspaceId, message.agentId);
        messages[key] = addMessage(messages[key] ?? [], message);
      }
      return { ...state, messages };
    }
    case 'sent': {
      const { userMessage, executionId } = action.sent;
      const key = runKey(userMessage.workspaceId, userMessage.agentId);
      const existing = state.runs[key];
      return {
        ...state,
        messages: { ...state.messages, [key]: addMessage(state.messages[key] ?? [], userMessage) },
        // Events can arrive before the command returns: keep the run they already created.
        runs:
          existing?.executionId === executionId
            ? state.runs
            : {
                ...state.runs,
                [key]: {
                  executionId,
                  status: 'running',
                  startedAt: userMessage.timestamp,
                  activity: [],
                  failureKind: null,
                  receiving: false,
                  pendingApprovalId: null,
                },
              },
        streams: without(state.streams, key),
        sendErrors: without(state.sendErrors, key),
      };
    }
    case 'sendFailed':
      return { ...state, sendErrors: { ...state.sendErrors, [action.key]: action.error } };
    case 'event': {
      const { event } = action;
      const key = runKey(event.workspaceId, event.agentId);
      // A finished execution has nothing left to approve, whatever became of its requests.
      const approvals = isTerminal(event.kind)
        ? state.approvals.filter((a) => a.executionId !== event.executionId)
        : applyApprovalEvent(state.approvals, event);
      const run = applyEvent(state.runs[key], event);
      if (!run) return { ...state, approvals };
      const streams =
        event.kind === 'output_chunk' &&
        run.executionId === event.executionId &&
        run.status === 'running'
          ? appendToStream(state.streams, key, event)
          : state.streams;
      return { ...state, approvals, runs: { ...state.runs, [key]: run }, streams };
    }
    case 'processStatus': {
      const { event } = action;
      const previous = state.processes[event.executionId];
      const process: ProcessState = {
        processSessionId: event.processSessionId,
        status: event.status,
        userAction: event.userAction,
        exitCode: event.exitCode,
        startedAt: previous?.startedAt ?? event.timestamp,
        endedAt: event.status === 'exited' ? event.timestamp : null,
      };
      return { ...state, processes: { ...state.processes, [event.executionId]: process } };
    }
    case 'approvalsLoaded':
      return { ...state, approvals: mergeApprovals(state.approvals, action.approvals) };
    case 'approvalGone':
      return { ...state, approvals: state.approvals.filter((a) => a.id !== action.id) };
    case 'message': {
      const { message } = action;
      const key = runKey(message.workspaceId, message.agentId);
      // The final answer replaces the live text of the same execution.
      const live = state.streams[key];
      const streams =
        message.role === 'assistant' && live?.executionId === message.executionId
          ? without(state.streams, key)
          : state.streams;
      return {
        ...state,
        streams,
        messages: { ...state.messages, [key]: addMessage(state.messages[key] ?? [], message) },
      };
    }
  }
}
