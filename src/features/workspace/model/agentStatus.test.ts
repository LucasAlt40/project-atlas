import type { RuntimeStatus } from '@/features/agents/types';
import type { AgentRun } from './agentRuns';
import { agentStatus } from './agentStatus';

const run = (
  overrides: Partial<AgentRun>,
  lastKind?: 'waiting_for_model' | 'sending_prompt',
): AgentRun => ({
  executionId: 'e',
  status: 'running',
  startedAt: 0,
  failureKind: null,
  pendingApprovalId: null,
  receiving: false,
  activity: lastKind ? [{ id: 'x', kind: lastKind, timestamp: 1, metadata: {} }] : [],
  ...overrides,
});

const runtime = (availability: RuntimeStatus['availability']): RuntimeStatus =>
  ({ availability }) as RuntimeStatus;

describe('agentStatus', () => {
  it('follows the execution while it runs, and only says waiting once the core does', () => {
    expect(agentStatus(run({}), undefined)).toBe('running');
    expect(agentStatus(run({}, 'sending_prompt'), undefined)).toBe('running');
    expect(agentStatus(run({}, 'waiting_for_model'), undefined)).toBe('waiting');
  });

  it('says responding only while answer text is arriving', () => {
    expect(agentStatus(run({ receiving: true }, 'waiting_for_model'), undefined)).toBe(
      'responding',
    );
    expect(agentStatus(run({ receiving: false }, 'waiting_for_model'), undefined)).toBe('waiting');
  });

  it('reports how the last execution ended', () => {
    expect(agentStatus(run({ status: 'completed' }), undefined)).toBe('completed');
    expect(agentStatus(run({ status: 'failed', failureKind: 'timeout' }), undefined)).toBe(
      'failed',
    );
    expect(
      agentStatus(run({ status: 'failed', failureKind: 'authentication_required' }), undefined),
    ).toBe('authentication_required');
  });

  it('falls back to the runtime availability when idle', () => {
    expect(agentStatus(undefined, runtime('ready'))).toBe('ready');
    expect(agentStatus(undefined, undefined)).toBe('ready');
    expect(agentStatus(undefined, runtime('authentication_required'))).toBe(
      'authentication_required',
    );
    expect(agentStatus(undefined, runtime('not_installed'))).toBe('unavailable');
    expect(agentStatus(undefined, runtime('unavailable'))).toBe('unavailable');
  });

  it('says stopping from the interrupt until the process is gone, then cancelled', () => {
    const process = (status: 'running' | 'interrupting' | 'terminating' | 'exited') => ({
      processSessionId: 'ps',
      status,
      userAction: null,
      exitCode: null,
      startedAt: 0,
      endedAt: null,
    });

    expect(agentStatus(run({}), undefined, process('running'))).toBe('running');
    expect(agentStatus(run({}), undefined, process('interrupting'))).toBe('stopping');
    expect(agentStatus(run({}), undefined, process('terminating'))).toBe('stopping');
    expect(agentStatus(run({ status: 'cancelled' }), undefined, process('exited'))).toBe(
      'cancelled',
    );
  });
});
