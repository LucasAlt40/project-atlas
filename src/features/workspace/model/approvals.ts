import type { PendingApprovalDto } from '@/lib/tauri/commands';
import type { ExecutionEvent } from '../types';

/** A command waiting for the user's decision. Everything here came from the core. */
export interface ApprovalRequest {
  id: string;
  workspaceId: string;
  agentId: string;
  executionId: string;
  /** The program and arguments as one line, for reading only. */
  command: string;
  cwd: string | null;
  /** Why Atlas asks, as a reason code the UI words. */
  reason: string | null;
}

const needsQuotes = /\s|^$/;

/** Joins program and arguments for display. Never used to run anything. */
export function commandLine(executable: string, args: string[]): string {
  return [executable, ...args]
    .map((part) => (needsQuotes.test(part) ? `"${part}"` : part))
    .join(' ');
}

export function fromPending(pending: PendingApprovalDto): ApprovalRequest {
  return {
    id: pending.id,
    workspaceId: pending.workspaceId,
    agentId: pending.agentId,
    executionId: pending.executionId,
    command: commandLine(pending.executable, pending.args),
    cwd: pending.cwd,
    reason: pending.reason,
  };
}

/** What a permission event does to the list of open requests. */
export function applyApprovalEvent(
  requests: ApprovalRequest[],
  event: ExecutionEvent,
): ApprovalRequest[] {
  if (event.kind !== 'permission') return requests;
  const id = event.metadata.approvalId;
  if (!id) return requests;
  switch (event.metadata.decision) {
    case 'approval_requested':
      if (requests.some((r) => r.id === id)) return requests;
      return [
        ...requests,
        {
          id,
          workspaceId: event.workspaceId,
          agentId: event.agentId,
          executionId: event.executionId,
          command: event.metadata.target ?? '',
          cwd: event.metadata.cwd ?? null,
          reason: event.metadata.reason ?? null,
        },
      ];
    case 'approved':
    case 'rejected':
      return requests.filter((r) => r.id !== id);
    default:
      return requests;
  }
}

/** Merges requests recovered from the core with those already known. */
export function mergeApprovals(
  known: ApprovalRequest[],
  recovered: ApprovalRequest[],
): ApprovalRequest[] {
  return [...known, ...recovered.filter((r) => !known.some((k) => k.id === r.id))];
}
