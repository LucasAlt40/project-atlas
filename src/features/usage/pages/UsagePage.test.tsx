import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { UsageMetricsDto, UsageRecordDto, UsageReportDto } from '@/lib/tauri/commands';
import { mockBackend, renderWithProviders } from '@/test/fixtures';
import { getUsageReport } from '../services/usageService';
import { UsagePage } from './UsagePage';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');

const NOW = Date.now();

function metrics(input: number, output: number, cost: number | null): UsageMetricsDto {
  return {
    inputTokens: input,
    outputTokens: output,
    totalTokens: input + output,
    cachedInputTokens: Math.round(input / 2),
    cost,
    currency: cost === null ? null : 'USD',
    source: 'runtime_reported',
  };
}

function run(id: string, over: Partial<UsageRecordDto>): UsageRecordDto {
  return {
    executionId: id,
    workspaceId: 'w1',
    agentId: 'a1',
    runtimeId: 'claude',
    modelId: 'sonnet',
    startedAt: NOW - 60_000,
    completedAt: NOW - 1_000,
    succeeded: true,
    metrics: metrics(10_000, 1_000, 0.12),
    ...over,
  };
}

const report: UsageReportDto = {
  connected: ['claude', 'codex', 'copilot'],
  records: [
    run('e1', {}),
    run('e2', {
      agentId: 'a2',
      runtimeId: 'codex',
      modelId: 'gpt-5.6-luna',
      metrics: metrics(40_000, 2_000, null),
    }),
    run('e3', { agentId: 'gone', succeeded: false, metrics: null }),
  ],
  agents: [
    { id: 'a1', name: 'Backend' },
    { id: 'a2', name: 'Reviewer' },
  ],
  workspaces: [{ id: 'w1', name: 'Atlas' }],
  runtimes: [
    { id: 'claude', name: 'Claude Code' },
    { id: 'codex', name: 'Codex' },
  ],
  quotas: {
    claude: {
      windows: [{ id: 'five_hour', usedFraction: 0.39, resetsAt: Math.round(NOW / 1000) + 3600 }],
      source: 'provider_reported',
      observedAt: NOW - 60_000,
      origin: 'atlas_run',
    },
    codex: {
      windows: [{ id: 'thirty_day', usedFraction: 0.5, resetsAt: Math.round(NOW / 1000) - 60 }],
      source: 'provider_reported',
      observedAt: NOW - 86_400_000,
      origin: 'cli_session',
      plan: 'free',
    },
  },
};

describe('the usage screen', () => {
  beforeEach(() => {
    mockBackend();
    vi.mocked(getUsageReport).mockResolvedValue(report);
  });

  it('shows the totals and says plainly what was not reported', async () => {
    renderWithProviders(<UsagePage />);

    expect(await screen.findByText('Reported cost')).toBeInTheDocument();
    // 11k + 42k tokens over the two executions that reported them.
    expect(screen.getByText('53K')).toBeInTheDocument();
    expect(screen.getByText(/No tokens reported for 1 of 3 executions/)).toBeInTheDocument();
    expect(screen.getByText(/Atlas never estimates a cost/)).toBeInTheDocument();
    expect(screen.getByText('from 1 of 3 executions')).toBeInTheDocument();
  });

  it('lists every agent, the largest first, and keeps a removed one', async () => {
    renderWithProviders(<UsagePage />);

    const table = await screen.findByRole('table');
    const names = within(table)
      .getAllByRole('button')
      .map((b) => b.getAttribute('aria-label'));
    expect(names).toEqual(['Open Reviewer', 'Open Backend', 'Open gone']);
    expect(within(table).getByText('removed')).toBeInTheDocument();
  });

  it('opens one agent on its own, with its models and latest executions', async () => {
    renderWithProviders(<UsagePage />);

    await userEvent.click(await screen.findByRole('button', { name: 'Open Backend' }));

    const drawer = await screen.findByRole('dialog');
    expect(within(drawer).getByText('Models used')).toBeInTheDocument();
    expect(within(drawer).getByText('Latest executions')).toBeInTheDocument();
    expect(within(drawer).getAllByText(/sonnet/).length).toBeGreaterThan(0);
  });

  it('says how much is left of each connected runtime, and where the number comes from', async () => {
    renderWithProviders(<UsagePage />);

    expect(await screen.findByText('What you have left')).toBeInTheDocument();
    // Claude: 39% used of 5 hours, read in an Atlas execution.
    expect(screen.getByRole('meter', { name: 'Claude Code · 5 hours' })).toHaveAttribute(
      'aria-valuenow',
      '0.61',
    );
    expect(screen.getByText('61% left')).toBeInTheDocument();
    expect(screen.getByText(/Reported by Claude Code in an Atlas execution/)).toBeInTheDocument();
    // Codex: read from its own session log; the window already reset since, which is said.
    expect(screen.getByText('plan free')).toBeInTheDocument();
    expect(
      screen.getByText(/Read from the latest Codex session on this computer/),
    ).toBeInTheDocument();
    expect(screen.getByText(/already reset/)).toBeInTheDocument();
    // Copilot is connected but reports no limit.
    expect(screen.getByText('copilot')).toBeInTheDocument();
    expect(screen.getByText('Does not report its limits.')).toBeInTheDocument();
  });
});
