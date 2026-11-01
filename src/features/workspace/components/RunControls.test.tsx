import { screen } from '@testing-library/react';
import { mockBackend, renderWithProviders } from '@/test/fixtures';
import type { AgentRun, ProcessState } from '../model/agentRuns';
import { RunControls } from './RunControls';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');

const NOW = 1_800_000_000_000;

function run(lastEventAt: number): AgentRun {
  return {
    executionId: 'exec-1',
    status: 'running',
    startedAt: NOW - 10 * 60 * 1000,
    activity: [],
    failureKind: null,
    receiving: false,
    pendingApprovalId: null,
    lastEventAt,
  };
}

const process: ProcessState = {
  processSessionId: 'p1',
  status: 'running',
  userAction: null,
  exitCode: null,
  startedAt: NOW - 10 * 60 * 1000,
  endedAt: null,
};

function show(lastEventAt: number) {
  renderWithProviders(
    <RunControls
      workspaceId="w"
      agentId="a"
      run={run(lastEventAt)}
      process={process}
      capabilities={undefined}
      onOpenTerminal={() => undefined}
    />,
  );
}

describe('the bar of a running execution', () => {
  beforeEach(() => {
    mockBackend();
    vi.useFakeTimers({ now: NOW, toFake: ['Date'] });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('says a run has had no output for minutes, so it does not look busy', async () => {
    show(NOW - 5 * 60 * 1000);
    expect(await screen.findByRole('status')).toHaveTextContent(/No output for 5/);
  });

  it('says nothing while output keeps coming', async () => {
    show(NOW - 30 * 1000);
    expect(await screen.findByText(/Elapsed/)).toBeInTheDocument();
    expect(screen.queryByText(/No output for/)).not.toBeInTheDocument();
  });
});
