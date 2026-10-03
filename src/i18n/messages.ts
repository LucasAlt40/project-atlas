import { isAppError, type AppErrorDto, type FailureKindDto } from '@/lib/tauri/commands';
import type { Translate, TranslationKey } from './index';

/** Words a structured error from the core in the user's language. */
export function appErrorMessage(t: Translate, error: AppErrorDto): string {
  const key = `error.${error.code}` as TranslationKey;
  return t(key, error.params);
}

/**
 * Any failure, as text for the user. A coded core error is translated; anything else
 * (an unexpected exception) falls back to a generic message with its text for diagnosis.
 */
export function errorMessage(t: Translate, error: unknown): string {
  const appError = isAppError(error)
    ? error
    : error instanceof Error && 'appError' in error && isAppError(error.appError)
      ? error.appError
      : undefined;
  if (appError) return appErrorMessage(t, appError);
  return t('error.generic');
}

const FAILURE_KINDS: readonly FailureKindDto[] = [
  'runtime_not_installed',
  'runtime_unavailable',
  'authentication_required',
  'model_unavailable',
  'timeout',
  'execution_failed',
  'invalid_request',
  'unexpected_response',
];

/** Words why an execution failed, from its failure kind. */
export function failureMessage(t: Translate, kind: string | null | undefined): string {
  const known = FAILURE_KINDS.find((k) => k === kind);
  return t(known ? `failure.${known}` : 'failure.unknown');
}
