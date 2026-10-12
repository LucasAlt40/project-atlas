import type { TranslationKey } from '@/i18n';
import type { InteractionKind, NodeStatus, RunStatus } from './types';

/** How a state is worded, and its tone. Colour is never the only signal: a glyph and words go with it. */
export type Tone = 'active' | 'attention' | 'good' | 'bad' | 'muted';

interface Look {
  key: TranslationKey;
  tone: Tone;
  glyph: string;
}

export const RUN_LOOK: Record<RunStatus, Look> = {
  running: { key: 'mobile.status.running', tone: 'active', glyph: '●' },
  paused: { key: 'mobile.status.paused', tone: 'muted', glyph: '❚❚' },
  waiting_for_input: { key: 'mobile.status.waitingForInput', tone: 'attention', glyph: '?' },
  completed: { key: 'mobile.status.completed', tone: 'good', glyph: '✓' },
  failed: { key: 'mobile.status.failed', tone: 'bad', glyph: '✕' },
  cancelled: { key: 'mobile.status.cancelled', tone: 'muted', glyph: '–' },
  interrupted: { key: 'mobile.status.interrupted', tone: 'attention', glyph: '!' },
};

export const STEP_LOOK: Record<NodeStatus, Look> = {
  pending: { key: 'mobile.step.pending', tone: 'muted', glyph: '○' },
  ready: { key: 'mobile.step.ready', tone: 'muted', glyph: '◔' },
  running: { key: 'mobile.step.running', tone: 'active', glyph: '●' },
  waiting_approval: { key: 'mobile.step.waitingApproval', tone: 'attention', glyph: '!' },
  waiting_for_input: { key: 'mobile.step.waitingForInput', tone: 'attention', glyph: '?' },
  completed: { key: 'mobile.step.completed', tone: 'good', glyph: '✓' },
  failed: { key: 'mobile.step.failed', tone: 'bad', glyph: '✕' },
  blocked: { key: 'mobile.step.blocked', tone: 'bad', glyph: '⊘' },
  cancelled: { key: 'mobile.step.cancelled', tone: 'muted', glyph: '–' },
  skipped: { key: 'mobile.step.skipped', tone: 'muted', glyph: '↷' },
};

export const KIND_KEY: Record<InteractionKind, TranslationKey> = {
  plan: 'mobile.kind.plan',
  clarification: 'mobile.kind.clarification',
  approval: 'mobile.kind.approval',
  permission: 'mobile.kind.permission',
  runtime_confirmation: 'mobile.kind.runtimeConfirmation',
};

/** The wording of a choice the core fixed (`approve`, `deny`…); anything else is the agent's own label. */
export const CHOICE_KEY: Partial<Record<string, TranslationKey>> = {
  approve: 'mobile.choice.approve',
  reject: 'mobile.choice.reject',
  allow: 'mobile.choice.allow',
  deny: 'mobile.choice.deny',
  yes: 'mobile.choice.yes',
  no: 'mobile.choice.no',
  revise: 'mobile.choice.revise',
  redo: 'mobile.choice.redo',
};

const FAILURE_KEYS: Record<string, TranslationKey> = {
  max_iterations_reached: 'mobile.failure.max_iterations_reached',
  node_failed: 'mobile.failure.node_failed',
  no_path_to_completion: 'mobile.failure.no_path_to_completion',
  no_route_matched: 'mobile.failure.no_route_matched',
  ended_in_failure: 'mobile.failure.ended_in_failure',
  invalid_workflow: 'mobile.failure.invalid_workflow',
  internal_error: 'mobile.failure.internal_error',
  worktree_unavailable: 'mobile.failure.worktree_unavailable',
};

/** How a run's failure is worded; a code from a newer Atlas is worded generically. */
export function failureKey(code: string): TranslationKey {
  return FAILURE_KEYS[code] ?? 'mobile.failure.unknown';
}
