import { isAppError } from '@/lib/tauri/commands';
import type { Translate, TranslationKey } from '@/i18n';
import { errorMessage } from '@/i18n/messages';

/** The reasons the core gives for refusing an import that are about the person's decisions. */
const DECISION_REASONS = [
  'decision_required',
  'no_existing',
  'dependency_unresolved',
  'binding_unknown',
  'unknown_item',
  'workspace_required',
  'workflow_running',
] as const;

/**
 * Why an import or export failed, in the person's words. The core refuses with a code and the
 * item it is about; the code is worded here, so the message says what to do about it.
 */
export function portabilityError(t: Translate, error: unknown): string {
  const appError = isAppError(error)
    ? error
    : error instanceof Error && 'appError' in error && isAppError(error.appError)
      ? error.appError
      : undefined;
  if (appError?.code === 'import_rejected') {
    const item = appError.params.item ?? '';
    const reason = appError.params.reason ?? 'invalid';
    const decision = DECISION_REASONS.find((r) => r === reason);
    if (decision) return t(`portability.reject.${decision}` as TranslationKey, { item });
    // A rule the item breaks, the same ones the editors enforce.
    return t('portability.reject.invalid_item', {
      item,
      reason: t(`portability.reason.${reason}` as TranslationKey),
    });
  }
  return errorMessage(t, error);
}

/** The words for an issue the core raised about an item, with its values filled in. */
export function issueText(
  t: Translate,
  issue: { code: string; params: Record<string, string> },
): string {
  const key = `portability.issue.${issue.code}` as TranslationKey;
  const reason = issue.params.reason;
  const params = {
    ...issue.params,
    ...(reason ? { reason: t(`portability.reason.${reason}` as TranslationKey) } : {}),
  };
  const text = t(key, params);
  return text === key ? issue.code : text;
}
