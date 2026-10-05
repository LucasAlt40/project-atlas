import { vi } from 'vitest';

/**
 * The Live Workspace service, quiet: a run has no live state and nothing is listening. For the
 * tests of the page around it; its own tests drive it.
 */
export function liveWorkspaceMock() {
  return {
    getLiveWorkspace: vi.fn().mockResolvedValue(null),
    refreshLiveWorkspace: vi.fn().mockRejectedValue(new Error('not mocked')),
    getLiveFile: vi.fn().mockRejectedValue(new Error('not mocked')),
    getLiveDiff: vi.fn().mockRejectedValue(new Error('not mocked')),
    subscribeToLiveUpdates: vi.fn().mockResolvedValue(() => undefined),
  };
}
