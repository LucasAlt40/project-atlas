import type { SpecView } from '../types';

export type CheckId =
  'documents' | 'structure' | 'consistency' | 'questions' | 'review' | 'findings' | 'approval';

export interface Check {
  id: CheckId;
  ok: boolean;
  /** The blockers that make it not ok, as the core named them. */
  blockers: SpecView['readiness']['blockers'];
}

const CODES: Record<CheckId, string[]> = {
  documents: ['artifact_not_written', 'artifact_not_approved', 'artifact_outdated'],
  structure: ['artifact_not_validated', 'artifact_has_errors'],
  consistency: ['consistency_not_run', 'consistency_errors'],
  questions: ['questions_pending'],
  review: ['review_not_run', 'review_outdated', 'review_running', 'review_failed'],
  findings: ['findings_open'],
  approval: ['final_approval_missing'],
};

export const CHECK_ORDER: readonly CheckId[] = [
  'documents',
  'structure',
  'consistency',
  'questions',
  'review',
  'findings',
  'approval',
];

/** The conditions of being ready, each with whether it holds and what is in its way. */
export function checksOf(view: SpecView): Check[] {
  return CHECK_ORDER.map((id) => {
    const blockers = view.readiness.blockers.filter((b) => CODES[id].includes(b.code));
    return { id, ok: blockers.length === 0, blockers };
  });
}

/** Everything but the person's last word holds: they may give it. */
export function canApproveFinal(view: SpecView): boolean {
  return (
    !view.spec.archived &&
    !view.busy &&
    view.readiness.blockers.every(
      (b) => b.code === 'final_approval_missing' || b.code.startsWith('publication_'),
    )
  );
}
