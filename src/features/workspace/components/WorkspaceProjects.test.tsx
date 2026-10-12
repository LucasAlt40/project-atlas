import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { CommandError, type AppErrorDto } from '@/lib/tauri/commands';
import { pickFolder } from '@/lib/tauri/dialog';
import {
  agent,
  chatMessage,
  emitProjectInit,
  mockBackend,
  renderWithProviders,
  workspace,
  workspaceWithProjects,
} from '@/test/fixtures';
import { WorkspacePage } from '../pages/WorkspacePage';
import {
  addWorkspaceProject,
  buildWorkspaceContext,
  initializeWorkspaceHarness,
  removeWorkspaceProject,
  saveWorkspaceNotes,
  sendMessage,
  updateWorkspaceProject,
} from '../services/workspaceService';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('../services/workspaceService');
vi.mock('@/lib/tauri/dialog');

const shop = () =>
  workspaceWithProjects(
    'w1',
    'Shop',
    [
      ['p-front', 'front', '/dev/shop/front', 'frontend'],
      ['p-api', 'API', '/dev/shop/api', 'backend'],
      ['p-orders', 'Orders', '/dev/shop/orders', 'service'],
    ],
    ['a1'],
  );

function show(options = {}) {
  mockBackend({
    agents: [agent('a1', 'Architect')],
    workspaces: [shop()],
    selectedWorkspaceId: 'w1',
    projectHarness: { 'p-front': 'ready', 'p-api': 'not_initialized', 'p-orders': 'outdated' },
    ...options,
  });
  return renderWithProviders(<WorkspacePage />);
}

function appError(code: AppErrorDto['code'], params: Record<string, string> = {}) {
  return new CommandError('add_workspace_project', { code, params, detail: null });
}

async function openPanel(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole('button', { name: 'Manage the projects of Shop' }));
  return screen.findByRole('dialog', { name: 'Projects of Shop' });
}

describe('the projects of a workspace', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows each project with its role and where its Harness stands, in one line', async () => {
    show();

    const summary = await screen.findByRole('list', { name: 'Projects of Shop' });

    await waitFor(() => {
      expect(
        within(summary)
          .getAllByRole('listitem')
          .map((i) => i.textContent),
      ).toEqual(['frontFrontendAnalysed', 'APIBackendNot analysed', 'OrdersServiceOut of date']);
    });
  });

  it('keeps the screen of a one-project workspace as it was: no list, no scope, a way to add more', async () => {
    mockBackend({
      agents: [agent('a1', 'Architect')],
      workspaces: [workspace('w1', 'ERP', '/dev/erp', ['a1'])],
      selectedWorkspaceId: 'w1',
    });
    renderWithProviders(<WorkspacePage />);

    expect(await screen.findByRole('group', { name: 'Harness' })).toBeVisible();
    expect(screen.getByRole('button', { name: 'Manage the projects of ERP' })).toBeVisible();
    expect(screen.queryByRole('list', { name: 'Projects of ERP' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^Scope:/ })).not.toBeInTheDocument();
  });

  it('lists the projects with their folders and offers what each state calls for', async () => {
    const user = userEvent.setup();
    show();

    const panel = await openPanel(user);
    const rows = await within(
      await within(panel).findByRole('list', { name: 'Registered projects' }),
    ).findAllByRole('listitem');

    expect(rows).toHaveLength(3);
    const [first, second, third] = rows as [HTMLElement, HTMLElement, HTMLElement];
    expect(within(first).getByText('/dev/shop/front')).toBeVisible();
    expect(within(first).getByText('Primary')).toBeVisible();
    // Analysed and out-of-date projects are refreshed; one never analysed is initialized.
    expect(within(first).getByRole('button', { name: 'Refresh Harness of front' })).toBeVisible();
    expect(within(second).getByRole('button', { name: 'Review Harness of API' })).toBeVisible();
    expect(within(third).getByRole('button', { name: 'Refresh Harness of Orders' })).toBeVisible();
  });

  it('adds a folder chosen with the native picker, and says so when it is already a project', async () => {
    const user = userEvent.setup();
    show();
    const panel = await openPanel(user);
    const added = {
      ...shop(),
      projects: [
        ...shop().projects,
        { id: 'p-new', name: 'billing', path: '/dev/shop/billing', role: null, dependsOn: [] },
      ],
    };
    vi.mocked(pickFolder).mockResolvedValueOnce('/dev/shop/billing');
    vi.mocked(addWorkspaceProject).mockResolvedValueOnce(added);

    await user.click(within(panel).getByRole('button', { name: 'Add project folder' }));

    expect(addWorkspaceProject).toHaveBeenCalledWith('w1', { path: '/dev/shop/billing' });
    expect(await screen.findByRole('list', { name: 'Projects of Shop' })).toHaveTextContent(
      'billing',
    );

    vi.mocked(pickFolder).mockResolvedValueOnce('/dev/shop/api');
    vi.mocked(addWorkspaceProject).mockRejectedValueOnce(
      appError('project_already_in_workspace', { path: '/dev/shop/api' }),
    );
    await user.click(within(panel).getByRole('button', { name: 'Add project folder' }));

    expect(await within(panel).findByRole('alert')).toHaveTextContent(
      'The folder “/dev/shop/api” is already a project of this workspace.',
    );
  });

  it('removes a project only after asking, and says its files stay', async () => {
    const user = userEvent.setup();
    show();
    const panel = await openPanel(user);
    const remaining = { ...shop(), projects: shop().projects.slice(0, 2) };
    vi.mocked(removeWorkspaceProject).mockResolvedValueOnce(remaining);

    await user.click(await within(panel).findByRole('button', { name: 'Remove Orders' }));

    expect(removeWorkspaceProject).not.toHaveBeenCalled();
    const ask = within(panel).getByRole('group', { name: 'Remove Orders' });
    expect(ask).toHaveTextContent(
      'Its folder, its files and its .atlas/ stay exactly where they are.',
    );

    await user.click(within(ask).getByRole('button', { name: 'Remove from workspace' }));

    expect(removeWorkspaceProject).toHaveBeenCalledWith('w1', 'p-orders');
    // The workspace the core returned is what the screen shows from here on.
    const summary = screen.getByRole('list', { name: 'Projects of Shop' });
    await waitFor(() => {
      expect(within(summary).getAllByRole('listitem')).toHaveLength(2);
    });
    expect(within(summary).queryByText('Orders')).not.toBeInTheDocument();
  });

  it('offers no way to remove the only project', async () => {
    const user = userEvent.setup();
    mockBackend({
      agents: [agent('a1', 'Architect')],
      workspaces: [workspace('w1', 'ERP', '/dev/erp', ['a1'])],
      selectedWorkspaceId: 'w1',
    });
    renderWithProviders(<WorkspacePage />);

    await user.click(await screen.findByRole('button', { name: 'Manage the projects of ERP' }));
    const panel = await screen.findByRole('dialog', { name: 'Projects of ERP' });

    expect(await within(panel).findByText('/dev/erp')).toBeVisible();
    expect(within(panel).queryByRole('button', { name: /^Remove / })).not.toBeInTheDocument();
    expect(
      within(panel).getByText('A workspace with one project has no workspace context.'),
    ).toBeVisible();
  });

  it('edits a project keeping its id, with a role and what it uses', async () => {
    const user = userEvent.setup();
    show();
    const panel = await openPanel(user);
    vi.mocked(updateWorkspaceProject).mockResolvedValueOnce(shop());

    await user.click(await within(panel).findByRole('button', { name: 'Edit API' }));
    const form = within(panel).getByRole('form', { name: 'Edit API' });
    await user.clear(within(form).getByLabelText('Name'));
    await user.type(within(form).getByLabelText('Name'), 'Billing API');
    await user.selectOptions(within(form).getByLabelText('Role'), 'bff');
    await user.click(within(form).getByRole('checkbox', { name: 'Orders' }));
    await user.click(within(form).getByRole('button', { name: 'Save project' }));

    expect(updateWorkspaceProject).toHaveBeenCalledWith('w1', 'p-api', {
      name: 'Billing API',
      path: '/dev/shop/api',
      role: 'bff',
      dependsOn: ['p-orders'],
    });
  });

  describe('starting the Harness of the projects', () => {
    it('starts only the projects that have none, shows each one working and reports the result', async () => {
      const user = userEvent.setup();
      show();
      const panel = await openPanel(user);
      let finish: (value: Awaited<ReturnType<typeof initializeWorkspaceHarness>>) => void = () =>
        undefined;
      vi.mocked(initializeWorkspaceHarness).mockReturnValueOnce(
        new Promise((resolve) => {
          finish = resolve;
        }),
      );

      await user.click(
        await within(panel).findByRole('button', { name: 'Start Harness of 1 project(s)' }),
      );

      expect(initializeWorkspaceHarness).toHaveBeenCalledWith('w1', ['p-api']);
      emitProjectInit({ workspaceId: 'w1', projectId: 'p-api', state: 'running' });
      expect(await within(panel).findByText('Analysing…')).toBeVisible();
      finish({
        projects: [{ projectId: 'p-api', status: 'initialized', error: null }],
        complete: true,
        context: { exists: true, generatedAt: 1, outdated: false, hasNotes: false },
      });
      expect(await within(panel).findByText('Every project has a Harness.')).toBeVisible();
    });

    it('never calls a partial run complete, names the project that failed and retries only it', async () => {
      const user = userEvent.setup();
      show({
        projectHarness: {
          'p-front': 'ready',
          'p-api': 'not_initialized',
          'p-orders': 'not_initialized',
        },
      });
      const panel = await openPanel(user);
      vi.mocked(initializeWorkspaceHarness).mockResolvedValueOnce({
        projects: [
          { projectId: 'p-api', status: 'initialized', error: null },
          {
            projectId: 'p-orders',
            status: 'failed',
            error: { code: 'project_analysis_failed', params: {}, detail: null },
          },
        ],
        complete: false,
        context: { exists: true, generatedAt: 1, outdated: false, hasNotes: false },
      });

      await user.click(
        await within(panel).findByRole('button', { name: 'Start Harness of 2 project(s)' }),
      );

      expect(initializeWorkspaceHarness).toHaveBeenCalledWith('w1', ['p-api', 'p-orders']);
      const status = await within(panel).findByRole('status');
      expect(status).toHaveTextContent('1 project(s) failed.');
      expect(status).toHaveTextContent('Orders:');
      expect(within(panel).queryByText('Every project has a Harness.')).not.toBeInTheDocument();

      vi.mocked(initializeWorkspaceHarness).mockResolvedValueOnce({
        projects: [{ projectId: 'p-orders', status: 'initialized', error: null }],
        complete: true,
        context: null,
      });
      await user.click(within(panel).getByRole('button', { name: 'Try the failed ones again' }));

      expect(initializeWorkspaceHarness).toHaveBeenLastCalledWith('w1', ['p-orders']);
    });
  });

  describe('the context of the workspace', () => {
    it('says when it was never built, when it is out of date, and rebuilds it on request', async () => {
      const user = userEvent.setup();
      show({
        workspaceContext: { exists: true, generatedAt: 5, outdated: true, hasNotes: false },
      });
      const panel = await openPanel(user);
      vi.mocked(buildWorkspaceContext).mockResolvedValueOnce({
        exists: true,
        generatedAt: 6,
        outdated: false,
        hasNotes: false,
      });
      const section = await within(panel).findByRole('region', { name: 'Workspace context' });

      expect(
        within(section).getByText('Out of date: a project, its role or its Harness changed.'),
      ).toBeVisible();
      await user.click(within(section).getByRole('button', { name: 'Rebuild context' }));

      expect(buildWorkspaceContext).toHaveBeenCalledWith('w1');
    });

    it('keeps the notes the user writes for the whole workspace', async () => {
      const user = userEvent.setup();
      show();
      const panel = await openPanel(user);
      vi.mocked(saveWorkspaceNotes).mockResolvedValueOnce(undefined);

      const notes = await within(panel).findByLabelText('Notes for the workspace');
      await waitFor(() => {
        expect(notes).toBeEnabled();
      });
      await user.type(notes, 'Orders owns the price rules.');
      await user.click(within(panel).getByRole('button', { name: 'Save notes' }));

      expect(saveWorkspaceNotes).toHaveBeenCalledWith('w1', 'Orders owns the price rules.');
      expect(await within(panel).findByText('Notes saved.')).toBeVisible();
    });
  });
});

describe('the scope of an agent', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
  });

  async function send(user: ReturnType<typeof userEvent.setup>, text: string) {
    vi.mocked(sendMessage).mockResolvedValue({
      userMessage: chatMessage('w1', 'a1', 'exec-1', 'user', text),
      executionId: 'exec-1',
    });
    await user.type(await screen.findByLabelText('Message Architect'), text);
    await user.click(screen.getByRole('button', { name: 'Send' }));
  }

  it('is the primary project alone unless it was chosen, and then nothing extra is sent', async () => {
    const user = userEvent.setup();
    show();

    await send(user, 'Review the checkout');

    expect(sendMessage).toHaveBeenCalledWith({
      workspaceId: 'w1',
      agentId: 'a1',
      content: 'Review the checkout',
    });
  });

  it('sends where the agent works and what it also consults, and remembers the choice', async () => {
    const user = userEvent.setup();
    show();

    await user.click(await screen.findByRole('button', { name: 'Scope: Works only in front.' }));
    await user.selectOptions(screen.getByRole('combobox', { name: 'Works in' }), 'p-api');
    await user.click(screen.getByRole('checkbox', { name: 'Orders' }));
    expect(
      screen.getByRole('button', { name: 'Scope: Works in API; consults Orders.' }),
    ).toBeVisible();
    await send(user, 'Add the field');

    expect(sendMessage).toHaveBeenCalledWith({
      workspaceId: 'w1',
      agentId: 'a1',
      content: 'Add the field',
      scope: { workingProjectId: 'p-api', readProjectIds: ['p-orders'] },
    });
  });

  it('can consult everything else at once and explains that only the working project changes', async () => {
    const user = userEvent.setup();
    show();

    await user.click(await screen.findByRole('button', { name: 'Scope: Works only in front.' }));
    await user.click(screen.getByRole('button', { name: 'Consult all' }));

    expect(
      screen.getByText(
        'Changes are made only in the project it works in. The others are read, never changed.',
      ),
    ).toBeVisible();
    await send(user, 'How do they relate?');
    expect(sendMessage).toHaveBeenCalledWith({
      workspaceId: 'w1',
      agentId: 'a1',
      content: 'How do they relate?',
      scope: { workingProjectId: 'p-front', readProjectIds: ['p-api', 'p-orders'] },
    });
  });
});
