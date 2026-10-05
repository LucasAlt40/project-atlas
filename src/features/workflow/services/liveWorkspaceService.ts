import { invokeCommand } from '@/lib/tauri/commands';
import type {
  LiveFileDto,
  LiveWorkspaceStateDto,
  LiveWorkspaceUpdateDto,
} from '@/lib/tauri/commands';
import { listenToEvent } from '@/lib/tauri/events';

/**
 * The Live Workspace: the files of a run's isolated worktree as they are now. Every call names
 * the run (and, for a file, a path inside its worktree); the folder is the backend's to choose.
 */

/** The snapshot. `null` when the run has no code worktree. */
export function getLiveWorkspace(executionId: string): Promise<LiveWorkspaceStateDto | null> {
  return invokeCommand('get_live_workspace', { executionId });
}

/** Compares the whole worktree with the baseline now. */
export function refreshLiveWorkspace(executionId: string): Promise<LiveWorkspaceStateDto> {
  return invokeCommand('refresh_live_workspace', { executionId });
}

/** One file of the worktree as it is now. */
export function getLiveFile(executionId: string, path: string): Promise<LiveFileDto> {
  return invokeCommand('get_live_file', { executionId, path });
}

/** The diff against the baseline, of one file or of everything. */
export function getLiveDiff(executionId: string, path?: string): Promise<string> {
  return invokeCommand('get_live_diff', { executionId, ...(path ? { path } : {}) });
}

/** Hears what changes in any run's worktree. Resolves with the way to stop listening. */
export function subscribeToLiveUpdates(
  handler: (update: LiveWorkspaceUpdateDto) => void,
): Promise<() => void> {
  return listenToEvent('live_workspace:changed', handler);
}
