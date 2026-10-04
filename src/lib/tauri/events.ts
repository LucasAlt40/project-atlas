import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  ExecutionEventDto,
  HarnessProgressDto,
  MessageDto,
  SessionStatusEventDto,
  TerminalChunkDto,
  WorkflowEventDto,
  WorkflowEventKindDto,
} from './commands';

/** The name of a workflow event on the wire: `workflow:node_started`. */
export type WorkflowEventName = `workflow:${WorkflowEventKindDto}`;

/** Every workflow event name, so a listener can subscribe to all of them. */
export const WORKFLOW_EVENT_NAMES: readonly WorkflowEventName[] = [
  'workflow:started',
  'workflow:paused',
  'workflow:resumed',
  'workflow:completed',
  'workflow:failed',
  'workflow:cancelled',
  'workflow:interrupted',
  'workflow:node_ready',
  'workflow:node_started',
  'workflow:node_waiting_approval',
  'workflow:node_approval_resolved',
  'workflow:node_waiting_for_input',
  'workflow:node_input_resolved',
  'workflow:interaction_detected',
  'workflow:interaction_answered',
  'workflow:interaction_rejected',
  'workflow:interaction_cancelled',
  'workflow:node_completed',
  'workflow:node_failed',
  'workflow:node_blocked',
  'workflow:node_skipped',
  'workflow:node_retrying',
  'workflow:artifact_created',
  'workflow:decision_created',
  'workflow:overlap_detected',
  'workflow:handoff_created',
  'workflow:integration_changed',
];

/**
 * Typed contract of the events the Rust core emits to the webview. Like `CommandMap`,
 * this is the only place the frontend touches Tauri's event API.
 */
export interface EventMap extends Record<WorkflowEventName, WorkflowEventDto> {
  /** Progress of a running execution; mirrors `commands::events`. */
  'execution:progress': ExecutionEventDto;
  /** A piece of a process's raw terminal output; mirrors `commands::events`. */
  'execution:output': TerminalChunkDto;
  /** A process changed state (running, interrupting, terminating, exited). */
  'execution:status': SessionStatusEventDto;
  /** What the agent analysing a project is doing. */
  'harness:progress': HarnessProgressDto;
  /** A message was added to a conversation; mirrors `commands::events`. */
  'conversation:message': MessageDto;
}

export type EventName = keyof EventMap;

export function listenToEvent<E extends EventName>(
  event: E,
  handler: (payload: EventMap[E]) => void,
): Promise<UnlistenFn> {
  return listen<EventMap[E]>(event, (e) => {
    handler(e.payload);
  });
}
