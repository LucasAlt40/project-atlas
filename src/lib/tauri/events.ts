import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { ExecutionEventDto, MessageDto } from './commands';

/**
 * Typed contract of the events the Rust core emits to the webview. Like `CommandMap`,
 * this is the only place the frontend touches Tauri's event API.
 */
export interface EventMap {
  /** Progress of a running execution; mirrors `commands::events`. */
  'execution:progress': ExecutionEventDto;
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
