import type { ReactNode } from 'react';
import { Badge, type BadgeTone } from './Badge';

/**
 * The one vocabulary of states of the app (docs/design-review/refatoracao-geral, part C). Every
 * feature maps its own statuses onto these, so the same state has the same color everywhere: a
 * badge, a canvas node, a list row, a header.
 */
export type StateKind =
  | 'ready'
  | 'running'
  | 'waiting'
  | 'paused'
  | 'interrupted'
  | 'completed'
  | 'failed'
  | 'cancelled'
  | 'pending'
  | 'unavailable'
  | 'off'
  | 'stale'
  | 'blocked';

export const STATE_TONE: Record<StateKind, BadgeTone> = {
  ready: 'success',
  running: 'running',
  waiting: 'warning',
  paused: 'idle',
  interrupted: 'warning',
  completed: 'success',
  failed: 'danger',
  cancelled: 'idle',
  pending: 'idle',
  unavailable: 'warning',
  off: 'idle',
  stale: 'warning',
  blocked: 'danger',
};

/** A state badge: the dot and the tone come from the state, the words from the caller. */
export function StateBadge({
  state,
  title,
  children,
}: {
  state: StateKind;
  title?: string;
  children: ReactNode;
}) {
  return (
    <Badge dot tone={STATE_TONE[state]} {...(title ? { title } : {})}>
      {children}
    </Badge>
  );
}
