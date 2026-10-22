import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {
  agent,
  harnessSummary,
  mockBackend,
  projectAnalysis,
  renderWithProviders,
  workspace,
} from '@/test/fixtures';
import { getWorkspaceHarness, refreshProjectHarness } from '../services/workspaceService';
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

/** The one place the Harness is handled: the projects panel, whatever the number of projects. */
async function openPanel(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole('button', { name: 'Manage the projects of ERP' }));
  return screen.findByRole('region', { name: 'Projects of ERP' });
}


/** "Recarregar" lives in the project's "⋯" menu: opens it and returns the item. */
async function refreshItem(
  user: ReturnType<typeof userEvent.setup>,
  scope: HTMLElement,
  name: string,
): Promise<HTMLElement> {
  await user.click(await within(scope).findByRole('button', { name: `More actions for ${name}` }));
  return within(scope).getByRole('menuitem', { name: 'Refresh' });
}

describe('Workspace Harness status', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('handles the Harness only in the panel, never on the workspace screen', async () => {
    const user = userEvent.setup();
    show({ harness: initialized });

    await screen.findByRole('button', { name: 'Manage the projects of ERP' });
    expect(screen.queryByRole('button', { name: 'Initialize Project' })).not.toBeInTheDocument();
    const panel = await openPanel(user);
    expect(await refreshItem(user, panel, 'erp')).toBeVisible();
  });

  it('says the project is not initialized and offers to initialize it', async () => {
    const user = userEvent.setup();
    show();

    const panel = await openPanel(user);
    expect(await within(panel).findByText('Not analysed')).toBeVisible();
    expect(within(panel).getByRole('button', { name: 'Review Harness of erp' })).toHaveTextContent(
      'Initialize Project',
    );
  });

  it('shows an initialized Harness with its stack and no initialize button', async () => {
    const user = userEvent.setup();
    show({ harness: initialized });

    const panel = await openPanel(user);
    expect(await within(panel).findByText(/Angular 21, \.NET 10/)).toBeVisible();
    expect(
      within(panel).queryByRole('button', { name: 'Review Harness of erp' }),
    ).not.toBeInTheDocument();
  });

  it('flags a Harness that needs review', async () => {
    const user = userEvent.setup();
    show({ harness: harnessSummary({ status: 'needs_review', hasAtlasDir: true }) });

    const panel = await openPanel(user);
    expect(
      await within(panel).findByRole('button', { name: 'Review Harness of erp' }),
    ).toHaveTextContent('Review Harness');
  });

  it('opens the full analysis and review from the panel', async () => {
    const user = userEvent.setup();
    show({ analysis: projectAnalysis() });

    const panel = await openPanel(user);
    await user.click(await within(panel).findByRole('button', { name: 'Review Harness of erp' }));
    expect(await screen.findByRole('region', { name: 'Initialize Project' })).toBeVisible();
  });

  it('shows how far the Harness can be trusted, and why', async () => {
    const user = userEvent.setup();
    show({
      harness: {
        ...initialized,
        health: { state: 'conflicted', reasons: ['unresolved_conflicts', 'partial_analysis'] },
      },
    });

    await openPanel(user);
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

    const panel = await openPanel(user);
    const notice = await within(panel).findByRole('status', { name: 'Harness stale' });
    expect(notice).toHaveTextContent('7 relevant project changes detected');
    expect(within(notice).getByText('package.json')).toBeVisible();
    expect(within(notice).getByText('… and 5 more')).toBeVisible();
    expect(notice).toHaveTextContent('Nothing was deleted');
    expect(await within(panel).findByLabelText(/Health: Stale/)).toHaveTextContent('Stale');
    expect(
      within(panel).getByText(/29 findings · 18 verified · 7 inferred · 3 may be outdated/),
    ).toBeVisible();

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
    await user.click(await refreshItem(user, panel, 'erp'));
    const dialog = await screen.findByRole('dialog', { name: 'Refresh Harness' });
    expect(await within(dialog).findByText(/1 relevant project change detected/)).toBeVisible();
    // Previewing writes nothing: only an explicit confirmation does.
    expect(vi.mocked(refreshProjectHarness)).toHaveBeenCalledTimes(1);
    expect(vi.mocked(refreshProjectHarness)).toHaveBeenCalledWith('w1', false, 'w1-main');
  });

  it('fixes a stale Harness in one click, applying the refresh without a preview', async () => {
    const user = userEvent.setup();
    show({
      harness: {
        ...initialized,
        health: { state: 'stale', reasons: ['project_changed'] },
        analyzedAt: Date.UTC(2026, 9, 3, 20, 14),
        staleness: {
          analyzedAt: Date.UTC(2026, 9, 3, 20, 14),
          totalChanges: 1,
          changes: [{ path: 'docs', kind: 'added' }],
        },
      },
    });
    vi.mocked(refreshProjectHarness).mockResolvedValue({
      staleness: null,
      diff: { added: [], removed: [], changed: [], unchanged: [] },
      conflicts: [],
      applied: {
        summary: { ...initialized, staleness: null, health: { state: 'healthy', reasons: [] } },
        written: [],
        backedUp: [],
        leftUntouched: [],
        gitIgnore: 'skipped',
      },
    });

    const panel = await openPanel(user);
    await user.click(await within(panel).findByRole('button', { name: 'Fix automatically' }));

    expect(vi.mocked(refreshProjectHarness)).toHaveBeenCalledWith('w1', true, 'w1-main');
  });

  it('does not claim staleness for a current Harness', async () => {
    const user = userEvent.setup();
    show({ harness: { ...initialized, health: { state: 'healthy', reasons: [] } } });

    await openPanel(user);
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
              gitIgnore: 'skipped',
            }
          : null,
      }),
    );

    const panel = await openPanel(user);
    await user.click(await refreshItem(user, panel, 'erp'));
    const dialog = await screen.findByRole('dialog', { name: 'Refresh Harness' });
    expect(await within(dialog).findByText('− Angular 21')).toBeVisible();
    expect(within(dialog).getByText('+ Angular 22')).toBeVisible();
    expect(
      within(dialog).getByText(/Your own files .* and your corrections are kept/),
    ).toBeVisible();
    // Previewing writes nothing.
    expect(refreshProjectHarness).toHaveBeenCalledTimes(1);
    expect(refreshProjectHarness).toHaveBeenCalledWith('w1', false, 'w1-main');

    await user.click(within(dialog).getByRole('button', { name: 'Apply changes' }));

    expect(refreshProjectHarness).toHaveBeenLastCalledWith('w1', true, 'w1-main');
    expect(await within(dialog).findByText(/Harness refreshed/)).toBeVisible();
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

    const panel = await openPanel(user);
    await user.click(await refreshItem(user, panel, 'erp'));

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

    const panel = await openPanel(user);
    await user.click(await refreshItem(user, panel, 'erp'));

    expect(await screen.findByRole('alert')).toHaveTextContent(/could not be read/);
  });

  it('says when the Harness state cannot be read', async () => {
    const user = userEvent.setup();
    show();
    vi.mocked(getWorkspaceHarness).mockRejectedValue(new Error('boom'));

    const panel = await openPanel(user);

    expect(await within(panel).findByRole('alert')).toHaveTextContent(
      'Could not read the projects of this workspace.',
    );
  });
});
