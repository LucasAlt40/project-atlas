import { failureMessage } from '@/i18n/messages';
import type { Translate } from '@/i18n';
import type { ExecutionEvent } from '../types';

/** One line of an agent's execution activity: what happened, not how it is worded. */
export interface ActivityEntry {
  /** Unique within the execution. */
  id: string;
  kind: ExecutionEvent['kind'];
  timestamp: number;
  /** `runtime` / `model` / `tool` names and `failureKind`, as the core sent them. */
  metadata: Record<string, string>;
}

export function toActivityEntry(event: ExecutionEvent, index: number): ActivityEntry {
  return {
    id: `${event.executionId}-${String(index)}`,
    kind: event.kind,
    timestamp: event.timestamp,
    metadata: event.metadata,
  };
}

export function isTerminal(kind: ExecutionEvent['kind']): boolean {
  return kind === 'completed' || kind === 'failed';
}

/** Words an activity entry in the user's language. Names (runtime, model, tool) stay as given. */
export function activityLabel(t: Translate, entry: ActivityEntry): string {
  if (entry.kind === 'failed') return failureMessage(t, entry.metadata.failureKind);
  return t(`agent.activity.${entry.kind}`, {
    runtime: entry.metadata.runtime ?? '',
    model: entry.metadata.model ?? '',
    tool: entry.metadata.tool ?? '',
  });
}
