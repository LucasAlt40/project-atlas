import type { Translate, TranslationKey } from '@/i18n';
import type { SpecArtifact } from '../types';
import type { SpecFinding, ValidationIssue } from '../types';
import type { SpecBlockerDto } from '@/lib/tauri/commands';

const ARTIFACT_KEYS = {
  proposal: 'specs.stage.proposal',
  specs: 'specs.stage.specs',
  design: 'specs.stage.design',
  tasks: 'specs.stage.tasks',
  final: 'specs.stage.final',
} as const satisfies Record<string, TranslationKey>;

/** The name of a stage or document, in the user's language. */
export function stageName(t: Translate, id: string): string {
  const key = (ARTIFACT_KEYS as Record<string, TranslationKey | undefined>)[id];
  return key ? t(key) : id;
}

/** `t(key)` when the dictionary has it, `fallback` when it does not (a code a newer core adds). */
function worded(
  t: Translate,
  key: string,
  fallback: string,
  params?: Record<string, string | number>,
): string {
  const text = t(key as TranslationKey, params);
  return text === key ? fallback : text;
}

/** One check's finding, in words. A code Atlas does not know yet shows the core's own text. */
export function issueText(t: Translate, issue: ValidationIssue): string {
  return worded(t, `specs.issue.${issue.code}`, issue.message, issue.params);
}

/** What keeps the spec from being ready, in words. */
export function blockerText(t: Translate, blocker: SpecBlockerDto): string {
  const params: Record<string, string | number> = {
    artifact: blocker.artifactId ? stageName(t, blocker.artifactId) : '',
    count: blocker.count ?? 0,
  };
  return worded(t, `specs.blocker.${blocker.code}`, blocker.code, params);
}

/**
 * Why a run did not produce a document or a review. The core sends a code, optionally followed
 * by `|` and a detail (a path, the runtime's own words).
 */
export function failureText(
  t: Translate,
  message: string,
): { text: string; detail: string | null } {
  const [code = '', ...rest] = message.split('|');
  const detail = rest.join('|').trim() || null;
  const text = worded(t, `specs.failure.${code}`, t('specs.failure.generic'), {
    detail: detail ?? '',
  });
  return { text, detail };
}

export function categoryText(t: Translate, finding: SpecFinding): string {
  return worded(t, `specs.finding.category.${finding.category}`, finding.category);
}

/** What the person has still to do about this document, if anything, as a sentence. */
export function artifactTitle(t: Translate, artifact: SpecArtifact): string {
  return stageName(t, artifact.id);
}
