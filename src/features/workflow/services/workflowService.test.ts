import { describe, expect, it, vi } from 'vitest';
import { listenToEvent } from '@/lib/tauri/events';
import { subscribeToWorkflowEvents } from './workflowService';

vi.mock('@/lib/tauri/events', async (importOriginal) => ({
  ...(await importOriginal<object>()),
  listenToEvent: vi.fn(),
}));

describe('subscribeToWorkflowEvents', () => {
  it('listens to every workflow event and stops them all at once', async () => {
    const stop = vi.fn();
    vi.mocked(listenToEvent).mockResolvedValue(stop);

    const unsubscribe = await subscribeToWorkflowEvents(() => undefined);
    const opened = vi.mocked(listenToEvent).mock.calls.length;
    unsubscribe();

    expect(opened).toBeGreaterThan(1);
    expect(stop).toHaveBeenCalledTimes(opened);
  });

  it('stops the listeners that opened when another one could not', async () => {
    vi.mocked(listenToEvent).mockReset();
    const stop = vi.fn();
    vi.mocked(listenToEvent).mockResolvedValue(stop).mockRejectedValueOnce(new Error('ipc closed'));

    await expect(subscribeToWorkflowEvents(() => undefined)).rejects.toThrow('ipc closed');

    const opened = vi.mocked(listenToEvent).mock.calls.length - 1;
    expect(stop).toHaveBeenCalledTimes(opened);
  });
});
