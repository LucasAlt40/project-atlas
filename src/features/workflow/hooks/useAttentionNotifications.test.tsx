import { act, render, waitFor } from '@testing-library/react';
import { chime, notifyUser } from '@/lib/tauri/notifications';
import { I18nProvider } from '@/i18n/I18nProvider';
import { pendingInteraction } from '@/test/workflowFixtures';
import { passwordRecovery, run, nodeState } from '@/test/workflowFixtures';
import {
  getRun,
  listPendingInteractions,
  subscribeToWorkflowEvents,
} from '../services/workflowService';
import type { WorkflowEvent } from '../types';
import { useAttentionNotifications, type AttentionTarget } from './useAttentionNotifications';

vi.mock('../services/workflowService');
vi.mock('@/lib/tauri/notifications');

let emit: (event: WorkflowEvent) => void = () => undefined;

function Probe({ onReturn }: { onReturn: (target: AttentionTarget) => void }) {
  useAttentionNotifications(onReturn);
  return null;
}

const asked = (): WorkflowEvent => ({
  kind: 'interaction_detected',
  workflowId: 'wf-1',
  executionId: 'wfx-1',
  workspaceId: 'w1',
  nodeId: 'developer',
  message: 'Which API should I use?',
  timestamp: 1,
  metadata: {},
});

function focused(value: boolean) {
  vi.spyOn(document, 'hasFocus').mockReturnValue(value);
}

describe('Notifying the person that an agent is waiting', () => {
  let onReturn: ReturnType<typeof vi.fn<(target: AttentionTarget) => void>>;

  beforeEach(async () => {
    vi.restoreAllMocks();
    vi.mocked(notifyUser).mockResolvedValue(true);
    vi.mocked(listPendingInteractions).mockResolvedValue([pendingInteraction()]);
    vi.mocked(subscribeToWorkflowEvents).mockImplementation((handler) => {
      emit = handler;
      return Promise.resolve(() => undefined);
    });
    onReturn = vi.fn();
    render(
      <I18nProvider language="en-US">
        <Probe onReturn={onReturn} />
      </I18nProvider>,
    );
    await act(() => Promise.resolve());
  });

  it('notifies with the question and the start of the plan when the app is in the background', async () => {
    focused(false);

    act(() => {
      emit(asked());
    });

    await waitFor(() => {
      expect(notifyUser).toHaveBeenCalledWith(
        expect.stringContaining('Developer'),
        expect.stringContaining('Plan · Add the column · Fix the webhook'),
      );
    });
    expect(vi.mocked(notifyUser).mock.calls[0]?.[1]).toContain('Which API should I use?');
  });

  it('still notifies with the question when the plan cannot be read', async () => {
    focused(false);
    vi.mocked(listPendingInteractions).mockRejectedValue(new Error('gone'));

    act(() => {
      emit(asked());
    });

    await waitFor(() => {
      expect(notifyUser).toHaveBeenCalledWith(expect.any(String), 'Which API should I use?');
    });
  });

  it('tells the person when a workflow completes, and whether there is code to review', async () => {
    focused(false);
    const done = run(passwordRecovery(), 'completed', {});
    vi.mocked(getRun).mockResolvedValue({
      ...done,
      integration: { ...done.integration, status: 'changes_available' },
    });

    act(() => {
      emit({ ...asked(), kind: 'completed' });
    });

    await waitFor(() => {
      expect(notifyUser).toHaveBeenCalledWith(
        expect.stringContaining('Workflow completed'),
        expect.stringContaining('ready for you to review'),
      );
    });
  });

  it('tells the person when a workflow fails, and why', async () => {
    focused(false);
    vi.mocked(getRun).mockResolvedValue(
      run(
        passwordRecovery(),
        'failed',
        { developer: nodeState('failed') },
        {
          failure: { code: 'node_failed', nodeId: 'developer', detail: null },
        },
      ),
    );

    act(() => {
      emit({ ...asked(), kind: 'failed' });
    });

    await waitFor(() => {
      expect(notifyUser).toHaveBeenCalledWith(
        expect.stringContaining('Workflow failed'),
        'Developer failed and nothing handles the failure.',
      );
    });
  });

  it('notifies with sound even while the app is in front, but does not send the person anywhere', async () => {
    focused(true);

    act(() => {
      emit(asked());
      emit({ ...asked(), kind: 'completed' });
    });

    await waitFor(() => {
      expect(notifyUser).toHaveBeenCalledTimes(2);
    });
    act(() => {
      window.dispatchEvent(new Event('focus'));
    });
    expect(onReturn).not.toHaveBeenCalled();
  });

  it('also chimes inside the app, whatever the system does with the notification', () => {
    focused(true);

    act(() => {
      emit({ ...asked(), kind: 'failed' });
    });

    expect(chime).toHaveBeenCalledTimes(1);
  });

  it('ignores every other workflow event', () => {
    focused(false);

    act(() => {
      emit({ ...asked(), kind: 'node_started' });
    });

    expect(notifyUser).not.toHaveBeenCalled();
  });

  it('takes the person to the run once, when they come back', () => {
    focused(false);
    act(() => {
      emit(asked());
    });

    focused(true);
    act(() => {
      window.dispatchEvent(new Event('focus'));
      window.dispatchEvent(new Event('focus'));
    });

    expect(onReturn).toHaveBeenCalledTimes(1);
    expect(onReturn).toHaveBeenCalledWith({ workflowId: 'wf-1', executionId: 'wfx-1' });
  });

  it('does nothing on return when nothing was asked', () => {
    focused(true);
    act(() => {
      window.dispatchEvent(new Event('focus'));
    });

    expect(onReturn).not.toHaveBeenCalled();
  });
});
