import type { TranslationKey } from '@/i18n';
import type { ArtifactPhase, SpecStatus, SpecSummary } from '../types';

export type Tone = 'neutral' | 'info' | 'running' | 'success' | 'warning' | 'danger';

interface View {
  key: TranslationKey;
  tone: Tone;
}

const STATUS: Record<SpecStatus, View> = {
  draft: { key: 'specs.status.draft', tone: 'neutral' },
  awaiting_generation: { key: 'specs.status.awaiting_generation', tone: 'neutral' },
  generating: { key: 'specs.status.generating', tone: 'running' },
  waiting_for_input: { key: 'specs.status.waiting_for_input', tone: 'warning' },
  awaiting_review: { key: 'specs.status.awaiting_review', tone: 'info' },
  changes_requested: { key: 'specs.status.changes_requested', tone: 'warning' },
  awaiting_validation: { key: 'specs.status.awaiting_validation', tone: 'info' },
  blocked: { key: 'specs.status.blocked', tone: 'danger' },
  approved: { key: 'specs.status.approved', tone: 'success' },
  ready_for_implementation: { key: 'specs.status.ready_for_implementation', tone: 'success' },
  published: { key: 'specs.status.published', tone: 'success' },
  archived: { key: 'specs.status.archived', tone: 'neutral' },
  failed: { key: 'specs.status.failed', tone: 'danger' },
};

const PHASE: Record<ArtifactPhase, View> = {
  blocked: { key: 'specs.phase.blocked', tone: 'neutral' },
  ready: { key: 'specs.phase.ready', tone: 'neutral' },
  generating: { key: 'specs.phase.generating', tone: 'running' },
  waiting_for_input: { key: 'specs.phase.waiting_for_input', tone: 'warning' },
  awaiting_review: { key: 'specs.phase.awaiting_review', tone: 'info' },
  changes_requested: { key: 'specs.phase.changes_requested', tone: 'warning' },
  approved: { key: 'specs.phase.approved', tone: 'success' },
  outdated: { key: 'specs.phase.outdated', tone: 'warning' },
  failed: { key: 'specs.phase.failed', tone: 'danger' },
};

export function statusView(status: SpecStatus): View {
  return STATUS[status];
}

export function phaseView(phase: ArtifactPhase): View {
  return PHASE[phase];
}

/** The groups of the overview: what the person is looking for when they filter the list. */
export type Group = 'all' | 'progress' | 'attention' | 'blocked' | 'ready' | 'archived';

export const GROUPS: readonly Group[] = [
  'all',
  'progress',
  'attention',
  'blocked',
  'ready',
  'archived',
];

/** Which group a status belongs to, besides `all`. */
export function groupOf(status: SpecStatus): Exclude<Group, 'all'> {
  switch (status) {
    case 'awaiting_review':
    case 'waiting_for_input':
    case 'awaiting_validation':
    case 'approved':
      return 'attention';
    case 'blocked':
    case 'failed':
      return 'blocked';
    case 'ready_for_implementation':
    case 'published':
      return 'ready';
    case 'archived':
      return 'archived';
    case 'draft':
    case 'awaiting_generation':
    case 'generating':
    case 'changes_requested':
      return 'progress';
  }
}

export function inGroup(spec: SpecSummary, group: Group): boolean {
  return group === 'all' ? spec.status !== 'archived' : groupOf(spec.status) === group;
}

export function countsByGroup(specs: SpecSummary[]): Record<Group, number> {
  const counts: Record<Group, number> = {
    all: 0,
    progress: 0,
    attention: 0,
    blocked: 0,
    ready: 0,
    archived: 0,
  };
  for (const spec of specs) {
    counts[groupOf(spec.status)] += 1;
    if (spec.status !== 'archived') counts.all += 1;
  }
  return counts;
}
