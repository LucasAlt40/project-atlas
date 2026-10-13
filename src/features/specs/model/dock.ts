import type { TranslationKey } from '@/i18n';
import type { SpecArtifact, SpecView } from '../types';

export type DockActionId = 'approve' | 'revise' | 'redo' | 'generate' | 'cancel' | 'keep';

export interface DockAction {
  id: DockActionId;
  enabled: boolean;
  /** Why it is off, when it is. */
  reason?: TranslationKey;
}

export interface Dock {
  /** What the person is being asked, or told, about this document. */
  message: TranslationKey;
  actions: DockAction[];
}

function action(id: DockActionId, reason?: TranslationKey): DockAction {
  return reason ? { id, enabled: false, reason } : { id, enabled: true };
}

/** The reason a run cannot be started now, if there is one. */
export function runBlocked(view: SpecView): TranslationKey | undefined {
  if (view.spec.archived) return 'specs.why.archived';
  if (!view.spec.started) return 'specs.why.not_started';
  if (view.busy) return 'specs.why.run_in_progress';
  if (!view.authorAvailable) return 'specs.why.author_missing';
  if (!view.projectAvailable) return 'specs.why.project_missing';
  return undefined;
}

/** Why the latest revision cannot be approved yet, if it cannot. */
export function approvalBlocked(
  view: SpecView,
  artifact: SpecArtifact,
): TranslationKey | undefined {
  const latest = artifact.revisions.at(-1);
  if (!latest) return 'specs.why.nothing_written';
  const validation = artifact.validation;
  if (validation?.revision !== latest.number) return 'specs.why.not_checked';
  if (validation.issues.some((issue) => issue.severity === 'error')) return 'specs.why.has_errors';
  if (view.pendingQuestions > 0) return 'specs.why.has_questions';
  return undefined;
}

/** What the dock of a document offers, from the document's state and the spec's. */
export function dockFor(view: SpecView, artifactId: string): Dock {
  const artifact = view.spec.artifacts.find((a) => a.id === artifactId);
  const phase = view.phases[artifactId];
  if (!artifact || !phase) return { message: 'specs.dock.nothing', actions: [] };
  const blocked = runBlocked(view);
  switch (phase) {
    case 'blocked':
      return { message: 'specs.dock.blocked', actions: [] };
    case 'ready':
      return { message: 'specs.dock.ready', actions: [action('generate', blocked)] };
    case 'generating':
      return { message: 'specs.dock.generating', actions: [action('cancel')] };
    case 'waiting_for_input':
      return { message: 'specs.dock.waiting_for_input', actions: [] };
    case 'awaiting_review': {
      const archived = view.spec.archived ? 'specs.why.archived' : undefined;
      return {
        message: 'specs.dock.awaiting_review',
        actions: [
          action('redo', archived ?? blocked),
          action('revise', archived ?? blocked),
          action('approve', archived ?? approvalBlocked(view, artifact)),
        ],
      };
    }
    case 'changes_requested':
      return { message: 'specs.dock.changes_requested', actions: [action('generate', blocked)] };
    case 'approved':
      return {
        message: 'specs.dock.approved',
        actions: [
          action('redo', view.spec.archived ? 'specs.why.archived' : blocked),
          action('revise', view.spec.archived ? 'specs.why.archived' : blocked),
        ],
      };
    case 'outdated':
      return {
        message: 'specs.dock.outdated',
        actions: [
          action(
            'keep',
            view.spec.archived
              ? 'specs.why.archived'
              : view.busy
                ? 'specs.why.run_in_progress'
                : undefined,
          ),
          action('generate', blocked),
        ],
      };
    case 'failed':
      return { message: 'specs.dock.failed', actions: [action('generate', blocked)] };
  }
}
