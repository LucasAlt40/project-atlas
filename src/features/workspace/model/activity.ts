import { failureMessage } from '@/i18n/messages';
import type { Translate, TranslationKey } from '@/i18n';
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

/** The execution is over: nothing more will happen to it. */
export function isTerminal(kind: ExecutionEvent['kind']): boolean {
  return kind === 'completed' || kind === 'failed' || kind === 'cancelled';
}

/** Something the *user* did (as opposed to the agent, the runtime or the process). */
export function isUserAction(kind: ExecutionEvent['kind']): boolean {
  return kind === 'user_interrupted' || kind === 'user_terminated';
}

/** Capabilities the runtime's own tools have beyond the policy, as the core listed them. */
function unenforcedOf(entry: ActivityEntry): string[] {
  return (entry.metadata.notes ?? '')
    .split(',')
    .filter((note) => note.startsWith('runtime_exceeds_policy:'))
    .map((note) => note.slice('runtime_exceeds_policy:'.length));
}

/** How a permission entry is shown: ✓ done, ✕ refused, ⚠ needs the user or a limit of Atlas. */
export function permissionTone(
  entry: ActivityEntry,
  pendingApprovalId: string | null,
): 'ok' | 'refused' | 'attention' {
  switch (entry.metadata.decision) {
    case 'denied':
    case 'rejected':
      return 'refused';
    case 'approval_requested':
      return entry.metadata.approvalId === pendingApprovalId ? 'attention' : 'ok';
    case 'allowed':
      return unenforcedOf(entry).length > 0 ? 'attention' : 'ok';
    default:
      return 'ok';
  }
}

function permissionLabel(t: Translate, entry: ActivityEntry): string {
  const { metadata } = entry;
  const params = {
    target: metadata.target ?? '',
    reason: metadata.reason ? t(`permission.reason.${metadata.reason}` as TranslationKey) : '',
  };
  switch (metadata.decision) {
    case 'allowed': {
      const unenforced = unenforcedOf(entry);
      const base = t(
        `agent.activity.permission.allowed.${metadata.action ?? 'run_process'}` as TranslationKey,
        params,
      );
      return unenforced.length === 0
        ? base
        : `${base} ${t('agent.activity.permission.unenforced', {
            what: unenforced.map((c) => t(`security.capability.${c}` as TranslationKey)).join(', '),
          })}`;
    }
    case 'denied':
      return t('agent.activity.permission.denied', params);
    case 'approval_requested':
      return t('agent.activity.permission.approval_requested', params);
    case 'approved':
      return t('agent.activity.permission.approved', params);
    case 'rejected':
      return t('agent.activity.permission.rejected', params);
    default:
      return t('agent.activity.permission', params);
  }
}

/** Words an activity entry in the user's language. Names (runtime, model, tool) stay as given. */
export function activityLabel(t: Translate, entry: ActivityEntry): string {
  if (entry.kind === 'failed') return failureMessage(t, entry.metadata.failureKind);
  if (entry.kind === 'permission') return permissionLabel(t, entry);
  if (entry.kind === 'process_exited') {
    const { exitCode, durationMs } = entry.metadata;
    return t(
      exitCode === undefined
        ? 'agent.activity.process_exited_unknown'
        : 'agent.activity.process_exited',
      { code: exitCode ?? '', duration: formatSeconds(durationMs) },
    );
  }
  return t(`agent.activity.${entry.kind}`, {
    runtime: entry.metadata.runtime ?? '',
    model: entry.metadata.model ?? '',
    tool: entry.metadata.tool ?? '',
    branch: entry.metadata.branch ?? '',
    merge: mergeText(t, entry.metadata.mergeStatus),
  });
}

/** The merge status an event carried, in words; empty when it carried none. */
function mergeText(t: Translate, status: string | undefined): string {
  return status === undefined ? '' : t(`git.merge.${status}` as TranslationKey);
}

/** `42s`, from a duration in milliseconds as text; empty when it was not reported. */
export function formatSeconds(milliseconds: string | undefined): string {
  const value = Number(milliseconds);
  if (milliseconds === undefined || !Number.isFinite(value)) return '';
  return `${String(value < 1000 ? Math.round(value / 100) / 10 : Math.round(value / 1000))}s`;
}
