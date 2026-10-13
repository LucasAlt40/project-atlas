import type { Translate } from '@/i18n';
import { stageName } from './messages';
import type { SpecSummary } from '../types';

/** "Next: Design", "Final review", or nothing to say, from where the spec is. */
export function phaseLabel(t: Translate, spec: SpecSummary): string {
  if (spec.status === 'archived') return t('specs.status.archived');
  if (spec.currentArtifact) {
    return t('specs.currentStage', { stage: stageName(t, spec.currentArtifact) });
  }
  if (spec.status === 'published' || spec.status === 'ready_for_implementation') {
    return t('specs.allApproved');
  }
  return t('specs.currentStage', { stage: stageName(t, 'final') });
}
