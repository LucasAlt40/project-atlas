import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {
  agent,
  harnessSummary,
  mockBackend,
  projectAnalysis,
  renderWithProviders,
  workspace,
} from '@/test/fixtures';
import { getProjectHarness, refreshProjectHarness } from '../services/workspaceService';
import { WorkspacePage } from '../pages/WorkspacePage';
import { WorkspaceSwitcher } from './WorkspaceSwitcher';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('../services/workspaceService');

const initialized = harnessSummary({
  status: 'initialized',
  projectName: 'Transport ERP',
  stack: ['Angular 21', '.NET 10'],
  version: 1,
  hasAtlasDir: true,
});

function show(options = {}) {
  mockBackend({
    agents: [agent('a1', 'Architect')],
    workspaces: [
      workspace('w1', 'ERP', '/dev/erp', ['a1']),
      workspace('w2', 'Other', '/dev/other'),
    ],
    selectedWorkspaceId: 'w1',
    ...options,
  });
  return renderWithProviders(
    <>
      <WorkspaceSwitcher pickFolder={vi.fn()} />
      <WorkspacePage />
    </>,
  );
}

describe('Workspace Harness status', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('says the project is not initialized and offers to initialize it', async () => {
    show();

    const harness = await screen.findByRole('group', { name: 'Harness' });
    expect(await within(harness).findByText(/Not initialized/)).toBeVisible();
    expect(within(harness).getByRole('button', { name: 'Initialize Project' })).toBeVisible();
  });

  it('shows an initialized Harness with its stack and no initialize button', async () => {
    show({ harness: initialized });

    const harness = await screen.findByRole('group', { name: 'Harness' });
    expect(await within(harness).findByText(/Initialized/)).toBeVisible();
    expect(within(harness).getByText('Angular 21 · .NET 10')).toBeVisible();
    expect(
      within(harness).queryByRole('button', { name: 'Initialize Project' }),
    ).not.toBeInTheDocument();
  });

  it('flags a Harness that needs review', async () => {
    show({ harness: harnessSummary({ status: 'needs_review', hasAtlasDir: true }) });

    const harness = await screen.findByRole('group', { name: 'Harness' });
    expect(await within(harness).findByText(/Needs review/)).toBeVisible();
    expect(within(harness).getByRole('button', { name: 'Review Harness' })).toBeVisible();
  });

  it('opens the analysis from the card and shows the new state once initialized', async () => {
    const user = userEvent.setup();
    show({ analysis: projectAnalysis() });

    await user.click(await screen.findByRole('button', { name: 'Initialize Project' }));
    expect(await screen.findByRole('dialog', { name: 'Initialize Project' })).toBeVisible();
  });

  it('shows how far the Harness can be trusted, and why', async () => {
    show({
      harness: {
        ...initialized,
        health: { state: 'conflicted', reasons: ['unresolved_conflicts', 'partial_analysis'] },
      },
    });

    const badge = await screen.findByLabelText(/Health: Conflicted/);
    expect(badge).toHaveTextContent('Conflicted');
    expect(badge).toHaveAccessibleName(/Evidence contradicts itself/);
    expect(badge).toHaveAccessibleName(/Part of the project could not be analysed/);
  });

  it('says the Harness is stale, what changed, that nothing was deleted, and offers a refresh', async () => {
    const user = userEvent.setup();
    show({
      harness: {
        ...initialized,
        health: { state: 'stale', reasons: ['project_changed'] },
        analyzedAt: Date.UTC(2026, 9, 3, 20, 14),
        stats: { findings: 29, verified: 18, inferred: 7, user: 0, stale: 3, unknown: 4 },
        staleness: {
          analyzedAt: Date.UTC(2026, 9, 3, 20, 14),
          totalChanges: 7,
          changes: [
            { path: 'package.json', kind: 'modified' },
            { path: 'src-tauri/Cargo.toml', kind: 'modified' },
          ],
        },
      },
    });

    const notice = await screen.findByRole('status', { name: 'Harness stale' });
    expect(notice).toHaveTextContent('7 relevant project changes detected');
    expect(within(notice).getByText('package.json')).toBeVisible();
    expect(within(notice).getByText('… and 5 more')).toBeVisible();
    expect(notice).toHaveTextContent('Nothing was deleted');
    expect(await screen.findByLabelText(/Health: Stale/)).toHaveTextContent('Stale');
    expect(
      screen.getByText(/29 findings · 18 verified · 7 inferred · 3 may be outdated/),
    ).toBeVisible();
    expect(screen.getByRole('button', { name: 'Refresh' })).toBeEnabled();

    vi.mocked(refreshProjectHarness).mockResolvedValue({
      staleness: {
        analyzedAt: 1,
        totalChanges: 1,
        changes: [{ path: 'package.json', kind: 'modified' }],
      },
      diff: { added: [], removed: [], changed: [], unchanged: [] },
      conflicts: [],
      applied: null,
    });
    await user.click(screen.getByRole('button', { name: 'Refresh' }));
    const dialog = await screen.findByRole('dialog');
    expect(await within(dialog).findByText(/1 relevant project change detected/)).toBeVisible();
    // Previewing writes nothing: only an explicit confirmation does.
    expect(vi.mocked(refreshProjectHarness)).toHaveBeenCalledTimes(1);
    expect(vi.mocked(refreshProjectHarness)).toHaveBeenCalledWith('w1', false);
  });

  it('does not claim staleness for a current Harness', async () => {
    show({ harness: { ...initialized, health: { state: 'healthy', reasons: [] } } });

    expect(await screen.findByLabelText(/Health: Healthy/)).toBeVisible();
    expect(screen.queryByRole('status', { name: 'Harness stale' })).not.toBeInTheDocument();
  });

  it('refresh previews the changes first and applies them only on confirmation', async () => {
    const user = userEvent.setup();
    show({ harness: initialized });
    const diff = {
      added: [],
      removed: [],
      changed: [
        { id: 'framework:angular', label: 'Angular 22', before: 'Angular 21', after: 'Angular 22' },
      ],
      unchanged: ['Docker'],
    };
    vi.mocked(refreshProjectHarness).mockImplementation((_id, confirm) =>
      Promise.resolve({
        staleness: null,
        diff,
        conflicts: [],
        applied: confirm
          ? {
              summary: { ...initialized, stack: ['Angular 22'] },
              written: [],
              backedUp: ['context/stack.md'],
              leftUntouched: [],
            }
          : null,
      }),
    );

    await user.click(await screen.findByRole('button', { name: 'Refresh' }));
    const dialog = await screen.findByRole('dialog', { name: 'Refresh Harness' });
    expect(await within(dialog).findByText('− Angular 21')).toBeVisible();
    expect(within(dialog).getByText('+ Angular 22')).toBeVisible();
    expect(
      within(dialog).getByText(/Your own files .* and your corrections are kept/),
    ).toBeVisible();
    // Previewing writes nothing.
    expect(refreshProjectHarness).toHaveBeenCalledTimes(1);
    expect(refreshProjectHarness).toHaveBeenCalledWith('w1', false);

    await user.click(within(dialog).getByRole('button', { name: 'Apply changes' }));

    expect(refreshProjectHarness).toHaveBeenLastCalledWith('w1', true);
    expect(await within(dialog).findByText(/Harness refreshed/)).toBeVisible();
    expect(await screen.findByText('Angular 22')).toBeVisible();
  });

  it('refresh shows conflicts the new analysis found', async () => {
    const user = userEvent.setup();
    show({ harness: initialized });
    vi.mocked(refreshProjectHarness).mockResolvedValue({
      staleness: null,
      diff: { added: [], removed: [], changed: [], unchanged: [] },
      conflicts: [
        {
          findingId: 'framework:angular',
          label: 'Angular 21',
          resolution: '19',
          claims: [
            { value: '19', choice: '19', origin: 'user_corrected', evidence: [{ source: 'user' }] },
            {
              value: 'Angular 21',
              choice: '21',
              origin: 'fact',
              evidence: [{ source: 'package.json' }],
            },
          ],
        },
      ],
      applied: null,
    });

    await user.click(await screen.findByRole('button', { name: 'Refresh' }));

    const conflicts = await screen.findByRole('region', { name: 'Conflicting information' });
    expect(within(conflicts).getByText('Decided: 19')).toBeVisible();
    expect(screen.getByText('Nothing changed.')).toBeVisible();
  });

  it('words a failed refresh', async () => {
    const user = userEvent.setup();
    show({ harness: initialized });
    vi.mocked(refreshProjectHarness).mockRejectedValue({
      code: 'harness_invalid',
      params: {},
      detail: null,
    });

    await user.click(await screen.findByRole('button', { name: 'Refresh' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(/could not be read/);
  });

  it('says when the Harness state cannot be read', async () => {
    show();
    vi.mocked(getProjectHarness).mockRejectedValue(new Error('boom'));
    // Switching workspace re-reads it; the failure is shown, not hidden.
    const user = userEvent.setup();
    await screen.findByRole('group', { name: 'Harness' });
    await user.click(screen.getByRole('button', { name: /^Workspace:/ }));
    await user.click(await screen.findByRole('option', { name: /Other/ }));

    await waitFor(() => {
      expect(screen.getByText('Could not read the Harness state.')).toBeVisible();
    });
  });

  it('reads the Harness of the workspace that is shown when switching', async () => {
    const user = userEvent.setup();
    show();
    vi.mocked(getProjectHarness).mockImplementation((id) =>
      Promise.resolve(id === 'w2' ? initialized : harnessSummary()),
    );
    await within(await screen.findByRole('group', { name: 'Harness' })).findByText(
      /Not initialized/,
    );

    await user.click(screen.getByRole('button', { name: /^Workspace:/ }));
    await user.click(await screen.findByRole('option', { name: /Other/ }));

    const harness = await screen.findByRole('group', { name: 'Harness' });
    expect(await within(harness).findByText(/Initialized/)).toBeVisible();
    expect(getProjectHarness).toHaveBeenCalledWith('w2');
  });
});
