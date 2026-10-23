import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {
  agent,
  mockBackend,
  navigate,
  renderWithProviders,
  workspace,
  workspaceWithProjects,
} from '@/test/fixtures';
import { summary } from '@/test/specFixtures';
import { passwordRecovery, run, templates } from '@/test/workflowFixtures';
import {
  getSpecProgress,
  listReadySpecs,
  subscribeToSpecChanges,
} from '@/features/specs/services/specService';
import {
  getRecovery,
  getRun,
  listIdes,
  listRuns,
  listTemplates,
  listWorkflows,
  planTemplateTools,
  selectTemplate,
  startWorkflow,
  subscribeToWorkflowEvents,
  updateWorkflow,
  validateWorkflow,
} from '../services/workflowService';
import { WorkflowPage } from './WorkflowPage';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');
vi.mock('@/features/specs/services/specService');
vi.mock('../services/workflowService');
vi.mock('../services/liveWorkspaceService', async () =>
  (await import('@/test/liveWorkspaceMock')).liveWorkspaceMock(),
);

function show(options: {
  ready?: ReturnType<typeof summary>[];
  projects?: [id: string, name: string, path: string][];
}) {
  const workflow = passwordRecovery();
  mockBackend({
    agents: [agent('a-architect', 'Architect agent')],
    workspaces: [
      options.projects
        ? workspaceWithProjects('w1', 'ERP', options.projects, ['a-architect'])
        : workspace('w1', 'ERP', '/dev/erp', ['a-architect']),
    ],
    selectedWorkspaceId: 'w1',
  });
  vi.mocked(listWorkflows).mockResolvedValue([workflow]);
  vi.mocked(listRuns).mockResolvedValue([]);
  vi.mocked(getRun).mockResolvedValue(null);
  vi.mocked(getRecovery).mockResolvedValue(null);
  vi.mocked(listTemplates).mockResolvedValue(templates);
  vi.mocked(listIdes).mockResolvedValue([]);
  vi.mocked(selectTemplate).mockResolvedValue('software_feature');
  vi.mocked(planTemplateTools).mockResolvedValue([]);
  vi.mocked(validateWorkflow).mockResolvedValue({ valid: true, issues: [] });
  vi.mocked(updateWorkflow).mockImplementation((w) => Promise.resolve(w));
  vi.mocked(subscribeToWorkflowEvents).mockResolvedValue(() => undefined);
  vi.mocked(subscribeToSpecChanges).mockResolvedValue(() => undefined);
  vi.mocked(listReadySpecs).mockResolvedValue(options.ready ?? []);
  vi.mocked(startWorkflow).mockReset();
  return { workflow };
}

beforeEach(() => {
  // Whether the task panel was left open is remembered: one test's must not reach the next.
  localStorage.clear();
  vi.clearAllMocks();
  navigate.mockReset();
});

/** How a run starts is a compact control of the run bar: opens it. */
async function openStart(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole('button', { name: /^How do you want to start\?/ }));
}

describe('starting a workflow: vibe coding or a spec', () => {
  it('runs as vibe coding by default: a spec is only used when chosen', async () => {
    show({ ready: [summary({ status: 'ready_for_implementation' })] });
    vi.mocked(startWorkflow).mockResolvedValue(run(passwordRecovery(), 'running', {}));
    const user = userEvent.setup();
    renderWithProviders(<WorkflowPage />);

    await screen.findByText(/The workflow is valid and can run/, {}, { timeout: 5000 });
    await user.type(await screen.findByLabelText('Task'), 'Implement password recovery');
    await openStart(user);
    expect(screen.getByRole('radio', { name: /Vibe coding/ })).toBeChecked();
    await user.click(screen.getByRole('button', { name: 'Run' }));

    expect(startWorkflow).toHaveBeenCalledWith('wf-1', 'Implement password recovery', false);
  });

  it('asks, points out a new ready spec, and keeps vibe coding as the default', async () => {
    show({
      ready: [summary({ id: 's1', title: 'Dark mode', status: 'ready_for_implementation' })],
    });
    vi.mocked(startWorkflow).mockResolvedValue(run(passwordRecovery(), 'running', {}));
    const user = userEvent.setup();
    renderWithProviders(<WorkflowPage />);

    await openStart(user);
    expect(await screen.findByText('How do you want to start?')).toBeInTheDocument();
    expect(
      await screen.findByText(/A new spec is ready to build: “Dark mode”/),
    ).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: /Vibe coding/ })).toBeChecked();

    await screen.findByText(/The workflow is valid and can run/, {}, { timeout: 5000 });
    await user.type(screen.getByLabelText('Task'), 'Implement password recovery');
    await user.click(screen.getByRole('button', { name: 'Run' }));

    // Vibe coding runs exactly as before: no spec on the call.
    expect(startWorkflow).toHaveBeenCalledWith('wf-1', 'Implement password recovery', false);
  });

  it('starts from the chosen spec, which needs no task text', async () => {
    show({
      ready: [
        summary({
          id: 's1',
          title: 'Dark mode',
          changeId: 'dark-mode',
          status: 'ready_for_implementation',
        }),
      ],
    });
    vi.mocked(startWorkflow).mockResolvedValue(run(passwordRecovery(), 'running', {}));
    const user = userEvent.setup();
    renderWithProviders(<WorkflowPage />);

    expect(await screen.findByText(/The workflow is valid and can run/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Run' })).toBeDisabled();
    await openStart(user);
    await user.click(await screen.findByRole('radio', { name: /From an approved spec/ }));
    expect(screen.getByLabelText('Spec')).toHaveValue('s1');
    expect(screen.getByRole('button', { name: 'Run' })).toBeEnabled();
    await user.click(screen.getByRole('button', { name: 'Run' }));

    expect(startWorkflow).toHaveBeenCalledWith('wf-1', '', false, undefined, 's1');
    // While the run goes the choice is not offered again.
    await waitFor(() => {
      expect(screen.queryByText('How do you want to start?')).not.toBeInTheDocument();
    });
  });

  it('says no spec is ready and offers to write one', async () => {
    show({ ready: [] });
    const user = userEvent.setup();
    renderWithProviders(<WorkflowPage />);

    await openStart(user);
    expect(await screen.findByText('No spec is ready yet.')).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: /From an approved spec/ })).toBeDisabled();
    await user.click(screen.getByRole('button', { name: 'Write a new spec' }));

    expect(navigate).toHaveBeenCalledWith('specs', { type: 'new-spec' });
  });

  it('shows what a run built from a spec has got through, reported and confirmed apart', async () => {
    const { workflow } = show({});
    const fromSpec = run(
      workflow,
      'running',
      {},
      {
        spec: {
          specId: 's1',
          title: 'Dark mode',
          changeId: 'dark-mode',
          revisions: {},
          digest: 'x',
          tasks: [
            { id: '1.1', text: 'Add the switch' },
            { id: '1.2', text: 'Persist it' },
          ],
          publishedRoot: null,
        },
      },
    );
    vi.mocked(listRuns).mockResolvedValue([fromSpec]);
    vi.mocked(getRun).mockResolvedValue(fromSpec);
    vi.mocked(getSpecProgress).mockResolvedValue([
      {
        workflowExecutionId: fromSpec.id,
        workflowName: workflow.name,
        status: 'running',
        specChanged: true,
        tasks: [
          { id: '1.1', text: 'Add the switch', state: 'reported' },
          { id: '1.2', text: 'Persist it', state: 'pending' },
        ],
        reported: 1,
        confirmed: 0,
      },
    ]);
    const user = userEvent.setup();
    renderWithProviders(<WorkflowPage />);

    const panel = await screen.findByRole('region', { name: /Built from the spec/ });
    // Closed it is one line of progress: the tasks are one click away.
    expect(
      await within(panel).findByText('0 confirmed · 1 reported · 2 tasks'),
    ).toBeInTheDocument();
    expect(within(panel).queryByText('Add the switch')).not.toBeInTheDocument();
    expect(
      within(panel).getByText(/The spec changed after this run was given it/),
    ).toBeInTheDocument();

    await user.click(within(panel).getByRole('button', { name: /Built from the spec/ }));
    expect(within(panel).getByText('Group 1')).toBeInTheDocument();
    expect(within(panel).getByText('1 of 2 reported')).toBeInTheDocument();
    expect(within(panel).getByText('Reported')).toBeInTheDocument();
    expect(within(panel).getByText('Pending')).toBeInTheDocument();
    expect(within(panel).getByText('Add the switch')).toBeInTheDocument();

    await user.click(within(panel).getByRole('button', { name: 'Open the spec' }));
    expect(navigate).toHaveBeenCalledWith('specs', { type: 'open-spec', specId: 's1' });
  });

  it('asks the same question on the form that makes a new workflow', async () => {
    show({
      ready: [summary({ id: 's1', title: 'Dark mode', status: 'ready_for_implementation' })],
    });
    vi.mocked(listWorkflows).mockResolvedValue([]);
    const user = userEvent.setup();
    renderWithProviders(<WorkflowPage />);

    const form = await screen.findByRole('form', { name: 'New workflow' });
    expect(within(form).getByText('How do you want to start?')).toBeInTheDocument();
    expect(within(form).getByRole('radio', { name: /Vibe coding/ })).toBeChecked();
    await user.click(within(form).getByRole('radio', { name: /From an approved spec/ }));
    expect(within(form).getByLabelText('Spec')).toHaveValue('s1');
  });
});

describe('running in a workspace of several projects (ADR 0043)', () => {
  const fullstack: [string, string, string][] = [
    ['back', 'api', '/dev/api'],
    ['front', 'web', '/dev/web'],
  ];

  it('changes only the primary project unless the person marks another', async () => {
    show({ projects: fullstack });
    vi.mocked(startWorkflow).mockResolvedValue(run(passwordRecovery(), 'running', {}));
    const user = userEvent.setup();
    renderWithProviders(<WorkflowPage />);

    await screen.findByText(/The workflow is valid and can run/, {}, { timeout: 5000 });
    await user.type(await screen.findByLabelText('Task'), 'Fix the API');
    await user.click(screen.getByRole('button', { name: 'Run' }));
    const dialog = await screen.findByRole('dialog', { name: 'Confirm the projects of this run' });
    expect(within(dialog).getByText(/Only api will be changed/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Run' }));

    expect(startWorkflow).toHaveBeenCalledWith('wf-1', 'Fix the API', false, {
      workingProjectId: 'back',
      readProjectIds: ['front'],
    });
  });

  it('says what will be changed before running, and runs once per marked project in order', async () => {
    show({ projects: fullstack });
    vi.mocked(startWorkflow).mockResolvedValue(run(passwordRecovery(), 'running', {}));
    const user = userEvent.setup();
    renderWithProviders(<WorkflowPage />);

    await screen.findByText(/The workflow is valid and can run/, {}, { timeout: 5000 });
    await user.type(await screen.findByLabelText('Task'), 'Add the orders screen');
    await user.click(screen.getByRole('button', { name: /^Projects of the run/ }));
    const web = screen.getByRole('group', { name: 'Use of web' });
    await user.click(within(web).getByRole('button', { name: 'Changes' }));
    await user.click(screen.getByRole('button', { name: 'Run' }));

    const dialog = await screen.findByRole('dialog', { name: 'Confirm the projects of this run' });
    expect(
      within(dialog).getByText('Changes, in order (one run per project):'),
    ).toBeInTheDocument();
    expect(startWorkflow).not.toHaveBeenCalled();
    await user.click(within(dialog).getByRole('button', { name: 'Run 2 runs' }));

    expect(startWorkflow).toHaveBeenCalledWith(
      'wf-1',
      'Add the orders screen',
      false,
      { workingProjectId: 'back', readProjectIds: [] },
      undefined,
      ['front'],
    );
  });

  it('runs in one project and consults the other when the person chose so', async () => {
    show({ projects: fullstack });
    vi.mocked(startWorkflow).mockResolvedValue(run(passwordRecovery(), 'running', {}));
    const user = userEvent.setup();
    renderWithProviders(<WorkflowPage />);

    await screen.findByText(/The workflow is valid and can run/, {}, { timeout: 5000 });
    await user.type(await screen.findByLabelText('Task'), 'Fix the API');
    await user.click(screen.getByRole('button', { name: /^Projects of the run/ }));
    const front = screen.getByRole('group', { name: 'Use of web' });
    await user.click(within(front).getByRole('button', { name: 'Consults' }));
    await user.click(screen.getByRole('button', { name: 'Run' }));
    const dialog = await screen.findByRole('dialog', { name: 'Confirm the projects of this run' });
    expect(within(dialog).getByText(/Only api will be changed/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Run' }));

    expect(startWorkflow).toHaveBeenCalledWith('wf-1', 'Fix the API', false, {
      workingProjectId: 'back',
      readProjectIds: ['front'],
    });
  });

  it('suggests changing one project when the workspace has many, and the person may keep all', async () => {
    show({
      projects: [
        ['a', 'a', '/a'],
        ['b', 'b', '/b'],
        ['c', 'c', '/c'],
        ['d', 'd', '/d'],
      ],
    });
    vi.mocked(startWorkflow).mockResolvedValue(run(passwordRecovery(), 'running', {}));
    const user = userEvent.setup();
    renderWithProviders(<WorkflowPage />);

    await screen.findByText(/The workflow is valid and can run/, {}, { timeout: 5000 });
    await user.type(await screen.findByLabelText('Task'), 'Rename the field');
    // By default only one project is changed; the person asks for all of them.
    await user.click(screen.getByRole('button', { name: /^Projects of the run/ }));
    await user.click(screen.getByRole('button', { name: 'Change all' }));
    await user.click(screen.getByRole('button', { name: 'Run' }));

    const dialog = await screen.findByRole('dialog', { name: 'Confirm the projects of this run' });
    expect(within(dialog).getByText('A workspace of 4 projects')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Change only a' }));

    expect(startWorkflow).toHaveBeenCalledWith('wf-1', 'Rename the field', false, {
      workingProjectId: 'a',
      readProjectIds: ['b', 'c', 'd'],
    });
  });
});
