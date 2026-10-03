import { isTerminal, toActivityEntry, type ActivityEntry } from './activity';
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
  status: 'running' | 'completed' | 'failed';
  /** Milliseconds since the Unix epoch when the run started. */
  startedAt: number;
  activity: ActivityEntry[];
  /** Set when the run failed; lets the UI tell "authentication required" from other failures. */
  failureKind: string | null;
  /** Answer text is arriving right now (cleared when a tool call or another step starts). */
  receiving: boolean;
}

/** The live answer of an execution that is still running. */
export interface LiveResponse {
  executionId: string;
  text: string;
}

export interface ConversationsState {
  messages: Record<string, Message[]>;
  runs: Record<string, AgentRun>;
  /** Text streamed so far for each running execution; replaced by its final message. */
  streams: Record<string, LiveResponse>;
  /** A rejected send (e.g. the agent is busy). */
  sendErrors: Record<string, unknown>;
}

export const initialConversations: ConversationsState = {
  messages: {},
  runs: {},
  streams: {},
  sendErrors: {},
};

export type ConversationAction =
  | { type: 'loaded'; messages: Message[] }
  | { type: 'sent'; sent: SentMessage }
  | { type: 'sendFailed'; key: string; error: unknown }
  | { type: 'event'; event: ExecutionEvent }
  | { type: 'message'; message: Message };

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

function applyEvent(run: AgentRun | undefined, event: ExecutionEvent): AgentRun | undefined {
  const sameExecution = run?.executionId === event.executionId;
  if (event.kind === 'output_chunk') {
    // Streamed text only means something inside the execution it belongs to.
    if (!run || !sameExecution) return run;
    if (run.receiving) return run;
    // The first piece of a stretch of text: note it once instead of once per token.
    const entry = toActivityEntry(event, run.activity.length);
    return { ...run, receiving: true, activity: [...run.activity, { ...entry, metadata: {} }] };
  }
  if (run && sameExecution) {
    // The same execution: append, and move to its final state on a terminal event.
    return {
      ...run,
      status:
        event.kind === 'completed' ? 'completed' : event.kind === 'failed' ? 'failed' : run.status,
      failureKind: event.kind === 'failed' ? failureKindOf(event) : run.failureKind,
      receiving: false,
      activity: [...run.activity, toActivityEntry(event, run.activity.length)],
    };
  }
  if (run?.status === 'running') {
    // A late event of an older execution while a newer one runs: not ours any more.
    return run;
  }
  // The agent was idle or finished: this is the start of a new execution.
  return {
    executionId: event.executionId,
    status: event.kind === 'failed' ? 'failed' : isTerminal(event.kind) ? 'completed' : 'running',
    startedAt: event.timestamp,
    failureKind: failureKindOf(event),
    receiving: false,
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
      const run = applyEvent(state.runs[key], event);
      if (!run) return state;
      const streams =
        event.kind === 'output_chunk' && run.executionId === event.executionId
          ? appendToStream(state.streams, key, event)
          : state.streams;
      return { ...state, runs: { ...state.runs, [key]: run }, streams };
    }
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
