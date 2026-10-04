import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { getAgentUsage } from '@/features/usage/services/usageService';
import {
  agent,
  chatMessage,
  claude,
  emit,
  emitConversationMessage,
  emptyAgentUsage,
  executionEvent,
  mockBackend,
  opencode,
  renderWithProviders,
  totals,
  workspace,
} from '@/test/fixtures';
import type { AgentUsageSummaryDto, UsageRecordDto } from '@/lib/tauri/commands';
import { sendMessage, setAgentPermissionProfile } from '../services/workspaceService';
import { WorkspacePage } from '../pages/WorkspacePage';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('../services/workspaceService');

const architect = agent('a1', 'Architect');
const developer = agent('a2', 'Developer', 'opencode', 'opencode/big-pickle');

function show(usage: AgentUsageSummaryDto = emptyAgentUsage(), options = {}) {
  mockBackend({
    agents: [architect, developer],
    workspaces: [workspace('w1', 'Atlas', '/dev/atlas', ['a1', 'a2'])],
    selectedWorkspaceId: 'w1',
    agentUsage: usage,
    ...options,
  });
  renderWithProviders(<WorkspacePage />);
}

async function openDetails(user: ReturnType<typeof userEvent.setup>, name: string) {
  await user.click(await screen.findByRole('button', { name: `Details of ${name}` }));
  return screen.getByRole('dialog', { name: `Details of ${name}` });
}

const record = (metrics: UsageRecordDto['metrics']): UsageRecordDto => ({
  executionId: 'exec-1',
  workspaceId: 'w1',
  agentId: 'a1',
  runtimeId: 'claude',
  modelId: 'sonnet',
  startedAt: 1_000,
  completedAt: 13_000,
  succeeded: true,
  metrics,
});

describe('Agent details and usage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('opens from the card header and shows who the agent is and its status', async () => {
    const user = userEvent.setup();
    show();

    const panel = await openDetails(user, 'Architect');

    expect(within(panel).getByRole('heading', { name: 'Architect' })).toBeInTheDocument();
    expect(within(panel).getByText(/Architect · Claude CLI · Anthropic/)).toBeInTheDocument();
    expect(within(panel).getAllByText('sonnet').length).toBeGreaterThan(0);
    expect(within(panel).getAllByText('Ready').length).toBeGreaterThan(0);
    expect(getAgentUsage).toHaveBeenCalledWith('w1', 'a1');
  });

  it('shows an agent as read only until it is given the developer profile', async () => {
    const user = userEvent.setup();
    show();

    const panel = await openDetails(user, 'Developer');

    const profile = await within(panel).findByRole('combobox', { name: 'Profile' });
    expect(profile).toHaveValue('read_only');
    expect(within(panel).getByText(/cannot edit files/)).toBeInTheDocument();

    await user.selectOptions(profile, 'developer');

    expect(setAgentPermissionProfile).toHaveBeenCalledWith('a2', 'developer');
  });

  it('does not say the agent is read only once it may write', async () => {
    const user = userEvent.setup();
    show(emptyAgentUsage(), { profile: 'developer' });

    const panel = await openDetails(user, 'Developer');

    expect(await within(panel).findByRole('combobox', { name: 'Profile' })).toHaveValue(
      'developer',
    );
    expect(within(panel).queryByText(/cannot edit files/)).not.toBeInTheDocument();
  });

  it('says "not reported" rather than showing zeros when nothing was reported', async () => {
    const user = userEvent.setup();
    show({
      ...emptyAgentUsage(),
      latestExecution: record(null),
      today: totals({ runs: 1 }),
      week: totals({ runs: 3 }),
      conversation: totals({ runs: 1 }),
    });

    const panel = await openDetails(user, 'Architect');

    const last = await within(panel).findByRole('region', { name: 'Last execution' });
    expect(within(last).getAllByText('Not reported')).toHaveLength(4);
    expect(within(last).queryByText('0')).not.toBeInTheDocument();
    const usage = within(panel).getByRole('region', { name: 'Usage' });
    expect(within(usage).getAllByText('Not reported').length).toBe(3);
    expect(within(usage).getByText(/3 runs/)).toBeInTheDocument();
  });

  it('shows the tokens, cost and sums the runtime reported, labelled as Atlas tracked', async () => {
    const user = userEvent.setup();
    show({
      latestExecution: record({
        inputTokens: 12_400,
        outputTokens: 3_800,
        totalTokens: 16_200,
        cost: 0.18,
        currency: 'USD',
        source: 'runtime_reported',
      }),
      conversation: totals({
        totalTokens: 20_000,
        cost: 0.31,
        currency: 'USD',
        runs: 2,
        runsWithTokens: 2,
        runsWithCost: 2,
      }),
      today: totals({
        totalTokens: 30_000,
        cost: 0.42,
        currency: 'USD',
        runs: 3,
        runsWithTokens: 3,
        runsWithCost: 3,
      }),
      week: totals({
        totalTokens: 90_000,
        cost: 2.31,
        currency: 'USD',
        runs: 9,
        runsWithTokens: 9,
        runsWithCost: 9,
      }),
      month: totals(),
      quota: null,
    });

    const panel = await openDetails(user, 'Architect');

    const last = await within(panel).findByRole('region', { name: 'Last execution' });
    expect(within(last).getByText('12.4K')).toBeInTheDocument();
    expect(within(last).getByText('3.8K')).toBeInTheDocument();
    expect(within(last).getByText('16.2K')).toBeInTheDocument();
    expect(within(last).getByText('$0.18')).toBeInTheDocument();
    expect(within(last).getByText(/Reported by the runtime/)).toBeInTheDocument();
    const usage = within(panel).getByRole('region', { name: 'Usage' });
    expect(within(usage).getByText('This conversation').nextSibling).toHaveTextContent('$0.31');
    expect(within(usage).getByText('Today').nextSibling).toHaveTextContent('$0.42');
    expect(within(usage).getByText('This week').nextSibling).toHaveTextContent('$2.31');
    expect(within(usage).getByText(/Atlas tracked\./)).toBeInTheDocument();
    expect(
      within(usage).getByText(/not your provider account usage or billing/),
    ).toBeInTheDocument();
  });

  it('a reported zero is shown as zero, not as "not reported"', async () => {
    const user = userEvent.setup();
    show({
      ...emptyAgentUsage(),
      latestExecution: record({
        inputTokens: 0,
        outputTokens: 0,
        totalTokens: 0,
        cost: 0,
        currency: null,
        source: 'runtime_reported',
      }),
    });

    const panel = await openDetails(user, 'Architect');

    const last = await within(panel).findByRole('region', { name: 'Last execution' });
    expect(within(last).queryByText('Not reported')).not.toBeInTheDocument();
    expect(within(last).getAllByText('0').length).toBe(3);
    expect(within(last).getByText('0.00')).toBeInTheDocument();
  });

  it('says how many runs a partial sum covers', async () => {
    const user = userEvent.setup();
    show({
      ...emptyAgentUsage(),
      today: totals({ cost: 0.5, currency: 'USD', runs: 4, runsWithCost: 1 }),
    });

    const panel = await openDetails(user, 'Architect');

    const usage = await within(panel).findByRole('region', { name: 'Usage' });
    expect(within(usage).getByText(/\$0\.50 \(reported for 1 of 4 runs\)/)).toBeInTheDocument();
  });

  it('shows provider-reported quota only for runtimes that report it, and says so for those that do not', async () => {
    const user = userEvent.setup();
    show({
      ...emptyAgentUsage(),
      quota: {
        windows: [
          { id: 'five_hour', usedFraction: 0.5, resetsAt: Math.floor(Date.now() / 1000) + 7200 },
          { id: 'seven_day', usedFraction: 0.07, resetsAt: null },
        ],
        source: 'provider_reported',
        observedAt: Date.now(),
      },
    });

    const panel = await openDetails(user, 'Architect');

    const quota = await within(panel).findByRole('region', { name: 'Quota' });
    expect(within(quota).getByRole('progressbar', { name: '5-hour window' })).toHaveAttribute(
      'aria-valuenow',
      '50',
    );
    expect(within(quota).getByRole('progressbar', { name: '7-day window' })).toHaveAttribute(
      'aria-valuenow',
      '7',
    );
    expect(within(quota).getByText('50% used')).toBeInTheDocument();
    expect(within(quota).getByText(/resets in 2 hours/)).toBeInTheDocument();
    expect(
      within(quota).getByText('Reported by the provider through Claude CLI.'),
    ).toBeInTheDocument();
  });

  it('says quota is not available for a runtime that does not expose it', async () => {
    const user = userEvent.setup();
    show();

    const panel = await openDetails(user, 'Developer');

    const quota = await within(panel).findByRole('region', { name: 'Quota' });
    expect(within(quota).getByText('Not available for this runtime.')).toBeInTheDocument();
    expect(within(quota).queryByRole('progressbar')).not.toBeInTheDocument();
  });

  it('says quota has not been reported yet for a runtime that reports it but has not run', async () => {
    const user = userEvent.setup();
    show();

    const panel = await openDetails(user, 'Architect');

    const quota = await within(panel).findByRole('region', { name: 'Quota' });
    expect(
      within(quota).getByText('Not reported yet. It appears after a run.'),
    ).toBeInTheDocument();
  });

  it('says usage is not available for a runtime that cannot report it', async () => {
    const user = userEvent.setup();
    const silent = {
      ...opencode,
      runtime: {
        ...opencode.runtime,
        capabilities: { ...opencode.runtime.capabilities, usageMetrics: false, costMetrics: false },
      },
    };
    show({ ...emptyAgentUsage(), latestExecution: record(null) }, { runtimes: [claude, silent] });

    const panel = await openDetails(user, 'Developer');

    const last = await within(panel).findByRole('region', { name: 'Last execution' });
    expect(within(last).getAllByText('Not available for this runtime')).toHaveLength(4);
  });

  it('while an agent runs, shows the current execution and its steps, with no invented counters', async () => {
    const user = userEvent.setup();
    show();
    vi.mocked(sendMessage).mockResolvedValue({
      userMessage: chatMessage('w1', 'a1', 'exec-1', 'user', 'Go'),
      executionId: 'exec-1',
    });
    await user.type(await screen.findByLabelText('Message Architect'), 'Go');
    await user.click(
      within(screen.getByRole('article', { name: 'Architect' })).getByRole('button', {
        name: 'Send',
      }),
    );
    act(() => {
      emit(executionEvent('w1', 'a1', 'exec-1', 'started'));
      emit(executionEvent('w1', 'a1', 'exec-1', 'waiting_for_model', { model: 'sonnet' }));
    });

    const panel = await openDetails(user, 'Architect');

    expect(within(panel).getByText(/^Running for \d+s$/)).toBeInTheDocument();
    const current = within(panel).getByRole('region', { name: 'Current execution' });
    expect(within(current).getByRole('list', { name: 'Activity' })).toHaveTextContent(
      'Waiting for sonnet',
    );
    expect(
      within(current).getByText('This runtime reports usage when the run finishes.'),
    ).toBeInTheDocument();
    expect(within(current).queryByText('Input')).not.toBeInTheDocument();
    expect(within(panel).getAllByText('Waiting for model').length).toBeGreaterThan(0);
  });

  it('refreshes the numbers when an execution finishes', async () => {
    const user = userEvent.setup();
    show();
    await openDetails(user, 'Architect');
    const calls = vi.mocked(getAgentUsage).mock.calls.length;

    act(() => {
      emit(executionEvent('w1', 'a1', 'exec-2', 'completed'));
      // The assistant message comes after the core recorded the execution's usage.
      emitConversationMessage(chatMessage('w1', 'a1', 'exec-2', 'assistant', 'done'));
    });

    await waitFor(() => {
      expect(vi.mocked(getAgentUsage).mock.calls.length).toBeGreaterThan(calls);
    });
  });

  it('shows the workspace summary and opens a broader view', async () => {
    const user = userEvent.setup();
    show(emptyAgentUsage(), {
      workspaceUsage: {
        today: totals({ cost: 0.84, currency: 'USD', runs: 2, runsWithCost: 2 }),
        week: totals({ cost: 3.12, currency: 'USD', runs: 5, runsWithCost: 5 }),
        month: totals({ cost: 9, currency: 'USD', runs: 20, runsWithCost: 20 }),
      },
    });

    const chip = await screen.findByRole('button', { name: 'Workspace usage' });
    expect(chip).toHaveTextContent('This workspace');
    expect(chip).toHaveTextContent('Today: $0.84');
    expect(chip).toHaveTextContent('Week: $3.12');
    await user.click(chip);

    const dialog = screen.getByRole('dialog', { name: 'Workspace usage' });
    expect(within(dialog).getByText('This month').nextSibling).toHaveTextContent('$9.00');
    expect(within(dialog).getByText(/Atlas tracked\./)).toBeInTheDocument();
  });
});

describe('Agent project context', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('says the project’s Harness is loaded and that it is shared and grants nothing', async () => {
    const user = userEvent.setup();
    show(undefined, {
      harness: {
        status: 'initialized',
        projectName: 'Atlas',
        stack: [],
        version: 1,
        initializedAt: 1,
        hasAtlasDir: true,
        problem: null,
      },
    });

    const panel = await openDetails(user, 'Architect');

    const context = within(panel).getByRole('region', { name: 'Project Context' });
    expect(await within(context).findByText('Harness: Loaded')).toBeVisible();
    expect(within(context).getByText(/grants no permissions/)).toBeVisible();
  });

  it('says when there is no Harness', async () => {
    const user = userEvent.setup();
    show();

    const panel = await openDetails(user, 'Architect');

    expect(await within(panel).findByText('Harness: Not initialized')).toBeVisible();
  });
});
