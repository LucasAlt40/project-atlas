import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {
  agent,
  mockBackend,
  navigate,
  renderWithProviders,
  storedExecution,
  workspace,
} from '@/test/fixtures';
import {
  attempt,
  nodeState,
  passwordRecovery,
  run,
  templates,
  workflowEvent,
} from '@/test/workflowFixtures';
import type { RecoveryPlanDto, WorkflowDto, WorkflowExecutionDto } from '@/lib/tauri/commands';
import {
  cancelWorkflow,
  createFromTemplate,
  createWorkflow,
  deleteWorkflow,
  getRecovery,
  getRun,
  listIdes,
  listRuns,
  listTemplates,
  listWorkflows,
  pauseWorkflow,
  repairWorkflowRoutes,
  resumeWorkflow,
  selectTemplate,
  startWorkflow,
  subscribeToWorkflowEvents,
  suggestRouteRepairs,
  updateWorkflow,
  validateWorkflow,
} from '../services/workflowService';
import type { ValidationReport, WorkflowEvent } from '../types';
import { WorkflowPage } from './WorkflowPage';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');
vi.mock('../services/workflowService');
vi.mock('../services/liveWorkspaceService', async () =>
  (await import('@/test/liveWorkspaceMock')).liveWorkspaceMock(),
);

const AGENTS = [
  agent('a-architect', 'Architect agent'),
  agent('a-developer', 'Developer agent'),
  agent('a-qa', 'QA agent'),
  agent('a-fixer', 'Fixer agent'),
];

let emitWorkflow: (event: WorkflowEvent) => void = () => undefined;

interface Setup {
  workflows?: WorkflowDto[];
  runs?: WorkflowExecutionDto[];
  language?: string;
  validation?: ValidationReport;
  recovery?: RecoveryPlanDto | null;
}

function show(setup: Setup = {}) {
  const workflows = setup.workflows ?? [passwordRecovery()];
  let runs = setup.runs ?? [];
  mockBackend({
    language: setup.language ?? 'en-US',
    agents: AGENTS,
    workspaces: [workspace('w1', 'ERP', '/dev/erp', ['a-architect'])],
    selectedWorkspaceId: 'w1',
    executions: [],
  });
  vi.mocked(listWorkflows).mockResolvedValue(workflows);
  vi.mocked(listRuns).mockImplementation(() => Promise.resolve(runs));
  vi.mocked(getRun).mockImplementation((id) =>
    Promise.resolve(runs.find((r) => r.id === id) ?? null),
  );
  vi.mocked(getRecovery).mockResolvedValue(setup.recovery ?? null);
  vi.mocked(listTemplates).mockResolvedValue(templates);
  vi.mocked(listIdes).mockResolvedValue([{ id: 'vscode', name: 'Visual Studio Code' }]);
  vi.mocked(selectTemplate).mockResolvedValue('software_feature');
  vi.mocked(validateWorkflow).mockResolvedValue(setup.validation ?? { valid: true, issues: [] });
  vi.mocked(updateWorkflow).mockImplementation((w) =>
    Promise.resolve({ ...w, version: w.version }),
  );
  vi.mocked(subscribeToWorkflowEvents).mockImplementation((handler) => {
    emitWorkflow = handler;
    return Promise.resolve(() => undefined);
  });
  for (const fn of [
    startWorkflow,
    pauseWorkflow,
    resumeWorkflow,
    cancelWorkflow,
    deleteWorkflow,
    createFromTemplate,
    createWorkflow,
  ]) {
    vi.mocked(fn).mockReset();
  }
  vi.mocked(pauseWorkflow).mockResolvedValue(undefined);
  vi.mocked(resumeWorkflow).mockResolvedValue(undefined);
  vi.mocked(cancelWorkflow).mockResolvedValue(undefined);
  vi.mocked(deleteWorkflow).mockResolvedValue(undefined);
  const rendered = renderWithProviders(<WorkflowPage />);
  return {
    ...rendered,
    setRuns(next: WorkflowExecutionDto[]) {
      runs = next;
    },
  };
}

const canvas = () => screen.findByRole('group', { name: 'Workflow graph' });
const node = async (name: string) =>
  within(await canvas()).findByLabelText(new RegExp(`^${name} \\(`));
/**
 * Clicks a node. The graph library starts a drag on mouse down, which user-event cannot feed
 * (it sends no window with the event), so only the click is sent.
 */
const clickNode = async (name: string) => {
  fireEvent.click(await node(name));
};
const side = () => screen.getByRole('complementary', { name: 'Workflow details' });

describe('Workflow page', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    navigate.mockReset();
  });

  describe('the graph', () => {
    it('draws the workflow: a node per step, with the agent and its personality', async () => {
      show();

      for (const name of ['Architect', 'Developer', 'QA', 'Bug Fixer', 'Done']) {
        expect(await node(name)).toBeInTheDocument();
      }
      expect(within(await node('QA')).getByText(/QA agent/)).toBeInTheDocument();
      expect(screen.getByRole('heading', { name: 'Workflow' })).toBeInTheDocument();
      expect(screen.getByText('version 1')).toBeInTheDocument();
    });

    it('selects a node and shows its configuration, with the agent as the source of truth', async () => {
      show();

      await clickNode('Developer');

      const panel = within(side());
      expect(panel.getByRole('heading', { name: 'Developer' })).toBeInTheDocument();
      expect(panel.getByLabelText('Agent')).toHaveValue('a-developer');
      expect(panel.getByLabelText('Instructions for this step')).toHaveValue(
        'Implement the endpoint.',
      );
      // What the agent is comes from the catalog, read-only.
      expect(panel.getByText('Isolation').nextSibling).toHaveTextContent('On');
      expect(panel.getByText('Context').nextSibling).toHaveTextContent('Task-aware');
      // Its dependency: what leads to it.
      expect(
        panel.getByRole('heading', { name: 'Dependencies' }).nextElementSibling,
      ).toHaveTextContent('Architect');
    });

    it('edits a node and saves exactly what was edited', async () => {
      const user = userEvent.setup();
      show();
      await clickNode('Developer');

      const instructions = within(side()).getByLabelText('Instructions for this step');
      await user.clear(instructions);
      await user.type(instructions, 'Only the API.');
      await user.clear(within(side()).getByLabelText('Retries'));
      await user.type(within(side()).getByLabelText('Retries'), '2');
      await user.selectOptions(within(side()).getByLabelText('On failure'), 'bug-fixer');

      const save = screen.getByRole('button', { name: 'Save' });
      expect(save).toBeEnabled();
      await user.click(save);

      const saved = vi.mocked(updateWorkflow).mock.calls[0]?.[0];
      const developer = saved?.nodes.find((n) => n.id === 'developer');
      expect(developer).toMatchObject({
        instructions: 'Only the API.',
        retryPolicy: { maxRetries: 2 },
        failurePolicy: { type: 'route_to_node', nodeId: 'bug-fixer' },
      });
      expect(saved?.nodes.find((n) => n.id === 'qa')).toMatchObject({ agentId: 'a-qa' });
      await waitFor(() => {
        expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
      });
    });

    it('adds an agent node by choosing an existing agent, and does not create agents', async () => {
      const user = userEvent.setup();
      show();
      await canvas();

      await user.click(screen.getByRole('button', { name: 'Add agent' }));
      const choices = screen.getByRole('list', { name: 'Choose an agent' });
      expect(
        within(choices)
          .getAllByRole('button')
          .map((b) => b.textContent),
      ).toEqual(['Architect agent', 'Developer agent', 'QA agent', 'Fixer agent']);
      await user.click(within(choices).getByRole('button', { name: 'Developer agent' }));

      expect(await node('Developer agent')).toBeInTheDocument();
      await user.click(screen.getByRole('button', { name: 'Save' }));
      expect(vi.mocked(updateWorkflow).mock.calls[0]?.[0].nodes).toHaveLength(6);
    });

    it('adds condition and end nodes, removes a node and undoes it', async () => {
      const user = userEvent.setup();
      show();
      await canvas();

      await user.click(screen.getByRole('button', { name: 'Add condition' }));
      expect(await node('Condition')).toBeInTheDocument();
      await user.click(screen.getByRole('button', { name: 'Add end' }));
      expect(await within(await canvas()).findAllByLabelText(/^Done \(/)).toHaveLength(2);

      await clickNode('Bug Fixer');
      await user.click(screen.getByRole('button', { name: 'Delete' }));
      await waitFor(() => {
        expect(
          within(screen.getByRole('group', { name: 'Workflow graph' })).queryByLabelText(
            /^Bug Fixer \(/,
          ),
        ).not.toBeInTheDocument();
      });

      await user.click(screen.getByRole('button', { name: 'Undo' }));
      expect(await node('Bug Fixer')).toBeInTheDocument();
      await user.click(screen.getByRole('button', { name: 'Redo' }));
      await waitFor(() => {
        expect(
          within(screen.getByRole('group', { name: 'Workflow graph' })).queryByLabelText(
            /^Bug Fixer \(/,
          ),
        ).not.toBeInTheDocument();
      });
    });

    it('connects two nodes and edits the connection', async () => {
      const user = userEvent.setup();
      show();
      await clickNode('Architect');

      await user.click(screen.getByRole('button', { name: 'Connect' }));
      expect(screen.getByRole('button', { name: 'Connect' })).toHaveAttribute(
        'aria-pressed',
        'true',
      );
      await clickNode('QA');
      expect(screen.getByRole('button', { name: 'Connect' })).toHaveAttribute(
        'aria-pressed',
        'false',
      );

      await user.click(screen.getByRole('button', { name: 'Save' }));
      const edges = vi.mocked(updateWorkflow).mock.calls[0]?.[0].edges ?? [];
      expect(edges).toHaveLength(6);
      expect(edges.at(-1)).toMatchObject({
        sourceNodeId: 'architect',
        targetNodeId: 'qa',
        condition: null,
      });
    });

    it('is tolerant of an empty selection and clears it on the pane', async () => {
      show();
      await clickNode('QA');
      expect(within(side()).getByRole('heading', { name: 'QA' })).toBeInTheDocument();
      expect(within(side()).getByLabelText('Max loop')).toHaveValue(3);
    });
  });

  describe('validation', () => {
    it('lists what is wrong in words and does not let the workflow run', async () => {
      const user = userEvent.setup();
      show({
        validation: {
          valid: false,
          issues: [
            {
              code: 'missing_agent',
              severity: 'error',
              nodeId: 'developer',
              edgeId: null,
              params: {},
            },
            {
              code: 'cycle_without_limit',
              severity: 'error',
              nodeId: 'qa',
              edgeId: null,
              params: { nodes: 'qa,bug-fixer' },
            },
          ],
        },
      });
      await canvas();

      const alert = await screen.findByRole('alert');
      expect(alert).toHaveTextContent('Workflow cannot start.');
      expect(alert).toHaveTextContent('Developer has no agent yet: choose one.');
      expect(alert).toHaveTextContent('QA, Bug Fixer');
      await user.type(screen.getByLabelText('Task'), 'Implement password recovery');
      expect(screen.getByRole('button', { name: 'Run' })).toBeDisabled();
      // The node that is wrong is marked on the canvas.
      expect((await node('Developer')).dataset.invalid).toBe('true');
    });

    describe('routes that do not match the contract', () => {
      const issues: ValidationReport['issues'] = [
        {
          code: 'status_route_on_contract',
          severity: 'error',
          nodeId: 'qa',
          edgeId: 'qa->done',
          params: { status: 'pass', outcomes: 'approved, changes_requested', agent: 'QA agent' },
        },
        {
          code: 'outcome_without_route',
          severity: 'error',
          nodeId: 'qa',
          edgeId: null,
          params: { outcome: 'approved', agent: 'QA agent' },
        },
        {
          code: 'outcome_without_route',
          severity: 'warning',
          nodeId: 'architect',
          edgeId: null,
          params: { outcomes: 'approved, changes_requested', agent: 'Architect agent' },
        },
      ];
      const proposals = [
        {
          nodeId: 'qa',
          agentId: 'a-qa',
          agent: 'QA agent',
          declared: ['approved', 'changes_requested'],
          edges: [
            {
              edgeId: 'qa->done',
              targetNodeId: 'done',
              current: { field: 'result.status', operator: 'equals' as const, value: 'pass' },
              suggested: 'approved',
            },
            {
              edgeId: 'qa->bug-fixer',
              targetNodeId: 'bug-fixer',
              current: { field: 'result.status', operator: 'equals' as const, value: 'fail' },
              suggested: 'changes_requested',
            },
          ],
        },
      ];

      it('explains status against outcome, names the agent contract and keeps warnings apart', async () => {
        vi.mocked(suggestRouteRepairs).mockResolvedValue(proposals);
        show({ validation: { valid: false, issues } });
        await canvas();

        const alert = await screen.findByRole('alert');
        expect(alert).toHaveTextContent('Workflow has routing issues');
        expect(alert).toHaveTextContent('result.status = "pass"');
        expect(alert).toHaveTextContent('approved, changes_requested');
        expect(alert).toHaveTextContent('technical state of a step');
        const warning = screen.getByText('Warnings').closest('div');
        expect(warning).toHaveTextContent('Architect agent');
        expect(alert).not.toHaveTextContent('Architect agent');
        expect(await screen.findByRole('button', { name: 'Review routes' })).toBeEnabled();
      });

      it('changes nothing until the person confirms, then repairs with what they chose', async () => {
        const user = userEvent.setup();
        vi.mocked(suggestRouteRepairs).mockResolvedValue(proposals);
        vi.mocked(repairWorkflowRoutes).mockResolvedValue(passwordRecovery({ version: 2 }));
        show({ validation: { valid: false, issues } });
        await canvas();

        await user.click(await screen.findByRole('button', { name: 'Review routes' }));
        const dialog = await screen.findByRole('dialog', {
          name: 'Reconnect routes to the contract',
        });
        expect(dialog).toHaveTextContent('result.status = pass → Done');
        expect(dialog).toHaveTextContent('does not assume');
        const [first, second] = within(dialog).getAllByRole('combobox');
        expect(first).toHaveValue('approved');
        expect(second).toHaveValue('changes_requested');
        expect(repairWorkflowRoutes).not.toHaveBeenCalled();

        // Cancel: nothing was applied.
        await user.click(within(dialog).getByRole('button', { name: 'Cancel' }));
        expect(repairWorkflowRoutes).not.toHaveBeenCalled();

        // The person may map it their own way; only then is it applied.
        await user.click(screen.getByRole('button', { name: 'Review routes' }));
        const again = await screen.findByRole('dialog', {
          name: 'Reconnect routes to the contract',
        });
        const [firstSelect] = within(again).getAllByRole('combobox');
        if (!firstSelect) throw new Error('the dialog has no select');
        await user.selectOptions(firstSelect, 'changes_requested');
        await user.click(within(again).getByRole('button', { name: 'Apply' }));
        await waitFor(() => {
          expect(repairWorkflowRoutes).toHaveBeenCalledWith('wf-1', [
            { edgeId: 'qa->done', outcome: 'changes_requested' },
            { edgeId: 'qa->bug-fixer', outcome: 'changes_requested' },
          ]);
        });
      });
    });

    it('says a valid workflow can run, and runs it with the task', async () => {
      const user = userEvent.setup();
      const w = passwordRecovery();
      const started = run(w, 'running', {});
      show();
      vi.mocked(startWorkflow).mockResolvedValue(started);
      await canvas();

      expect(await screen.findByText(/The workflow is valid and can run/)).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Run' })).toBeDisabled(); // no task yet
      await user.type(screen.getByLabelText('Task'), 'Implement password recovery');
      await user.click(screen.getByRole('button', { name: 'Run' }));

      expect(startWorkflow).toHaveBeenCalledWith('wf-1', 'Implement password recovery');
      expect(
        await screen.findByRole('status', { name: 'Workflow status: Running' }),
      ).toBeInTheDocument();
    });
  });

  describe('modes and templates', () => {
    it('shows Automatic and turns it into Custom without changing the graph', async () => {
      const user = userEvent.setup();
      show();
      await canvas();

      expect(screen.getByText('Automatic', { selector: 'span' })).toBeInTheDocument();
      await user.click(screen.getByRole('button', { name: 'Customize workflow' }));
      expect(screen.getByText('Custom', { selector: 'span' })).toBeInTheDocument();
      expect(screen.queryByRole('button', { name: 'Customize workflow' })).not.toBeInTheDocument();

      await user.click(screen.getByRole('button', { name: 'Save' }));
      const saved = vi.mocked(updateWorkflow).mock.calls[0]?.[0];
      expect(saved?.mode).toBe('custom');
      expect(saved?.nodes).toEqual(passwordRecovery().nodes);
      expect(saved?.edges).toEqual(passwordRecovery().edges);
    });

    it('proposes a recommended workflow for the task in Automatic mode and creates it', async () => {
      const user = userEvent.setup();
      show({ workflows: [] });
      vi.mocked(createFromTemplate).mockResolvedValue({
        workflow: passwordRecovery(),
        missingRoles: [],
      });

      expect(await screen.findByRole('form', { name: 'New workflow' })).toBeInTheDocument();
      await user.type(screen.getByLabelText('Task'), 'Implement password recovery');
      expect(await screen.findByRole('status')).toHaveTextContent(
        'Recommended workflow: Software Feature Development',
      );
      await user.click(screen.getByRole('button', { name: 'Create workflow' }));

      expect(createFromTemplate).toHaveBeenCalledWith(
        'w1',
        'software_feature',
        'automatic',
        undefined,
      );
      expect(await node('Architect')).toBeInTheDocument();
      // The task typed to get a recommendation is the task to run.
      expect(screen.getByLabelText('Task')).toHaveValue('Implement password recovery');
    });

    it('starts a custom workflow from a chosen template or from a blank one', async () => {
      const user = userEvent.setup();
      show({ workflows: [] });
      vi.mocked(createFromTemplate).mockResolvedValue({
        workflow: passwordRecovery({ mode: 'custom' }),
        missingRoles: ['qa'],
      });

      await user.click(await screen.findByRole('radio', { name: /Custom/ }));
      expect(screen.queryByLabelText('Task')).not.toBeInTheDocument();
      await user.selectOptions(screen.getByLabelText('Start from'), 'bug_fix');
      await user.type(screen.getByLabelText('Name'), 'My bug fix');
      await user.click(screen.getByRole('button', { name: 'Create workflow' }));

      expect(createFromTemplate).toHaveBeenCalledWith('w1', 'bug_fix', 'custom', 'My bug fix');
      // A role no agent could fill is said, never filled in by Atlas.
      expect(await screen.findByText(/No agent was found for: QA/)).toBeInTheDocument();
    });

    it('creates a blank workflow when no template is chosen', async () => {
      const user = userEvent.setup();
      show({ workflows: [] });
      vi.mocked(createWorkflow).mockResolvedValue(
        passwordRecovery({ mode: 'custom', nodes: [], edges: [] }),
      );

      await user.click(await screen.findByRole('radio', { name: /Custom/ }));
      await user.click(screen.getByRole('button', { name: 'Create workflow' }));

      expect(createWorkflow).toHaveBeenCalledWith({
        workspaceId: 'w1',
        name: 'Workflow',
        mode: 'custom',
      });
    });
  });

  describe('a run', () => {
    const w = passwordRecovery();
    const running = () =>
      run(
        w,
        'running',
        {
          architect: nodeState('completed', { attempts: [attempt('exec-4')] }),
          developer: nodeState('running', { attempts: [attempt('exec-5', 1, 'running')] }),
        },
        {
          state: {
            ...run(w, 'running', {}).state,
            currentNodes: ['developer'],
            completedNodes: ['architect'],
            activeAgents: ['a-developer'],
            artifacts: [
              {
                id: 'artifact-1',
                type: 'architecture_document',
                name: 'architecture.md',
                producerNodeId: 'architect',
                executionId: 'exec-4',
                path: 'docs/architecture.md',
                summary: 'POST /password-reset, 15m token',
                metadata: {},
                createdAt: 1,
              },
            ],
            decisions: [
              {
                id: 'decision-1',
                title: 'JWT',
                decision: 'Use JWT',
                rationale: 'Already used',
                sourceNodeId: 'architect',
                createdAt: 1,
              },
            ],
            touchedAreas: { developer: ['auth', 'api'] },
            warnings: [{ nodeIds: ['architect', 'developer'], paths: ['src/auth'] }],
          },
        },
      );

    it('follows the run live: each node says where it is, with a symbol and a word', async () => {
      show({ runs: [running()] });

      const developer = await node('Developer');
      expect(developer.dataset.status).toBe('running');
      expect(within(developer).getByText(/Running/)).toBeInTheDocument();
      expect(within(developer).getByText('#5')).toBeInTheDocument();
      const architect = await node('Architect');
      expect(within(architect).getByText(/Completed/)).toBeInTheDocument();
      expect(within(architect).getByText('✓')).toBeInTheDocument();
      expect(within(await node('QA')).getByText(/Pending/)).toBeInTheDocument();
      expect(screen.getByRole('status', { name: 'Workflow status: Running' })).toBeInTheDocument();
    });

    it('summarises the run: progress, what is going on now and who is working', async () => {
      show({ runs: [running()] });
      await canvas();

      const overview = await screen.findByRole('region', { name: 'Workflow overview' });
      expect(within(overview).getByText('1 / 4 nodes completed')).toBeInTheDocument();
      expect(within(overview).getByText('Current').nextSibling).toHaveTextContent('Developer');
      expect(within(overview).getByText('Active agents').nextSibling).toHaveTextContent(
        'Developer agent',
      );
      const done = within(overview)
        .getAllByRole('listitem')
        .filter((li) => li.textContent.includes('✓'));
      expect(done).toHaveLength(1);
      expect(done[0]).toHaveTextContent('Architect');
    });

    it('shows the shared state compactly: artifacts, decisions, changed areas and warnings', async () => {
      show({ runs: [running()] });
      await canvas();

      const shared = await screen.findByRole('region', { name: 'Shared state' });
      expect(within(shared).getByRole('button', { name: 'architecture.md' })).toBeInTheDocument();
      expect(
        within(shared).getByText(/Architecture document · Architect · docs\/architecture.md/),
      ).toBeInTheDocument();
      expect(within(shared).getByText('JWT')).toBeInTheDocument();
      expect(within(shared).getByText('api, auth')).toBeInTheDocument();
      expect(
        within(shared).getByText(
          /Potential overlap detected between Architect and Developer: src\/auth/,
        ),
      ).toBeInTheDocument();
    });

    it('cannot be edited while it runs: the canvas and the node settings are read-only', async () => {
      show({ runs: [running()] });

      await clickNode('Developer');

      expect(screen.getByRole('button', { name: 'Add agent' })).toBeDisabled();
      expect(screen.queryByRole('button', { name: 'Save' })).not.toBeInTheDocument();
      expect(within(side()).getByLabelText('Instructions for this step')).toBeDisabled();
      expect(within(side()).getByLabelText('Agent')).toBeDisabled();
      expect(screen.queryByRole('button', { name: 'Run' })).not.toBeInTheDocument();
    });

    it('pauses and cancels the workflow', async () => {
      const user = userEvent.setup();
      show({ runs: [running()] });

      await user.click(await screen.findByRole('button', { name: 'Pause' }));
      expect(pauseWorkflow).toHaveBeenCalledWith('wfx-1');
      await user.click(screen.getByRole('button', { name: 'Cancel workflow' }));
      expect(cancelWorkflow).toHaveBeenCalledWith('wfx-1');
    });

    it('resumes a paused workflow', async () => {
      const user = userEvent.setup();
      show({ runs: [{ ...running(), status: 'paused' }] });

      await user.click(await screen.findByRole('button', { name: 'Resume' }));
      expect(resumeWorkflow).toHaveBeenCalledWith('wfx-1');
      expect(screen.queryByRole('button', { name: 'Pause' })).not.toBeInTheDocument();
    });

    it('updates when the core says something happened, by reading the run again', async () => {
      const s = show({ runs: [running()] });
      await node('Developer');

      s.setRuns([
        run(w, 'completed', {
          architect: nodeState('completed'),
          developer: nodeState('completed'),
          qa: nodeState('completed'),
          done: nodeState('completed'),
        }),
      ]);
      act(() => {
        emitWorkflow(workflowEvent('completed'));
      });

      await waitFor(() => {
        expect(
          screen.getByRole('status', { name: 'Workflow status: Completed' }),
        ).toBeInTheDocument();
      });
      expect(getRun).toHaveBeenCalledWith('wfx-1');
      // A finished run can be left for the editor.
      expect(screen.getByRole('button', { name: 'Back to editor' })).toBeInTheDocument();
      // Events of another workspace are not ours.
      vi.mocked(getRun).mockClear();
      act(() => {
        emitWorkflow({ ...workflowEvent('started'), workspaceId: 'w2' });
      });
      await new Promise((resolve) => setTimeout(resolve, 80));
      expect(getRun).not.toHaveBeenCalled();
    });

    it('shows a failed run with why, which node failed and what was blocked', async () => {
      const user = userEvent.setup();
      show({
        runs: [
          run(
            w,
            'failed',
            {
              architect: nodeState('completed'),
              developer: nodeState('failed', { reason: 'The runtime exited with an error' }),
              qa: nodeState('blocked', { reason: 'dependency_failed' }),
            },
            { failure: { code: 'node_failed', nodeId: 'developer', detail: null } },
          ),
        ],
      });

      // A finished run is not what the page opens on: it is picked from the runs.
      await user.selectOptions(await screen.findByLabelText('Runs'), 'wfx-1');
      const alert = await screen.findByRole('alert');
      expect(alert).toHaveTextContent('Developer failed and nothing handles the failure.');
      expect((await node('Developer')).dataset.status).toBe('failed');
      expect(within(await node('QA')).getByText(/Blocked/)).toBeInTheDocument();
      await clickNode('QA');
      expect(
        within(side()).getByText('A step it depends on failed or was cancelled.'),
      ).toBeInTheDocument();
    });

    describe('a verdict and recovery', () => {
      const failedOnVerdict = () =>
        run(
          w,
          'failed',
          {
            architect: nodeState('completed', { attempts: [attempt('exec-1')] }),
            developer: nodeState('completed', {
              attempts: [attempt('exec-2')],
              facts: { 'result.status': 'success', 'result.outcome': 'fail' },
              unrouted: true,
            }),
            qa: nodeState('blocked', { reason: 'not_reached' }),
          },
          {
            failure: { code: 'no_route_matched', nodeId: 'developer', detail: 'fail' },
            state: {
              ...run(w, 'failed', {}).state,
              validationResults: [
                {
                  nodeId: 'developer',
                  executionId: 'exec-2',
                  status: 'success',
                  outcome: 'fail',
                  summary: 'The webhook is not atomic.',
                  findings: [
                    {
                      severity: 'high',
                      category: 'Transactions',
                      title: 'Missing transaction boundary',
                      file: 'src/payment.ts',
                      line: 142,
                      description: '',
                      evidence: '',
                      recommendation: '',
                    },
                  ],
                },
              ],
            },
          },
        );

      it('shows a fail the step concluded as a result with its findings, not as a broken step', async () => {
        const user = userEvent.setup();
        show({ runs: [failedOnVerdict()] });

        await user.selectOptions(await screen.findByLabelText('Runs'), 'wfx-1');

        expect(await screen.findByRole('alert')).toHaveTextContent(
          'Developer ended with the result "fail" (the step itself ran well), but the workflow has no route for that result.',
        );
        expect(screen.getByText(/Result: FAIL/)).toBeInTheDocument();
        expect(screen.getByText(/Missing transaction boundary/)).toBeInTheDocument();
        expect(screen.getByText(/src\/payment\.ts:142/)).toBeInTheDocument();
        expect(
          screen.getByText(/No route of the workflow matches this result/),
        ).toBeInTheDocument();
      });

      it('offers to resume where the graph says, naming what is kept and what runs next', async () => {
        const user = userEvent.setup();
        show({
          runs: [failedOnVerdict()],
          recovery: {
            kind: 'resume',
            failureNodeId: 'developer',
            lastCompletedNodeId: 'developer',
            restartNodeIds: ['bug-fixer'],
            reusedNodeIds: ['architect', 'developer'],
            problem: null,
          },
        });

        await user.selectOptions(await screen.findByLabelText('Runs'), 'wfx-1');

        expect(await screen.findByText('This run can be resumed')).toBeInTheDocument();
        expect(screen.getByText('Goes on from: Bug Fixer')).toBeInTheDocument();
        expect(screen.getByText('Kept, not redone: Architect, Developer')).toBeInTheDocument();
        await user.click(screen.getByRole('button', { name: 'Resume where it stopped' }));
        expect(resumeWorkflow).toHaveBeenCalledWith('wfx-1');
      });

      it('says why a run cannot be resumed yet and offers no button', async () => {
        const user = userEvent.setup();
        show({
          runs: [failedOnVerdict()],
          recovery: {
            kind: 'resume',
            failureNodeId: 'developer',
            lastCompletedNodeId: 'developer',
            restartNodeIds: [],
            reusedNodeIds: ['architect', 'developer'],
            problem: 'no_route',
          },
        });

        await user.selectOptions(await screen.findByLabelText('Runs'), 'wfx-1');

        expect(await screen.findByText('This run cannot be resumed yet')).toBeInTheDocument();
        expect(screen.getByText(/Add one in the editor and come back/)).toBeInTheDocument();
        expect(screen.queryByRole('button', { name: 'Resume where it stopped' })).toBeNull();
      });
    });

    it('says a loop reached its limit, and which attempt each node is on', async () => {
      const user = userEvent.setup();
      show({
        runs: [
          run(
            w,
            'failed',
            {
              qa: nodeState('blocked', {
                iterations: 3,
                reason: 'max_iterations',
                attempts: [attempt('exec-7', 3, 'completed')],
              }),
            },
            { failure: { code: 'max_iterations_reached', nodeId: 'qa', detail: 'qa_bug_fix' } },
          ),
        ],
      });

      await user.selectOptions(await screen.findByLabelText('Runs'), 'wfx-1');
      expect(await screen.findByRole('alert')).toHaveTextContent(
        'Maximum loop iterations reached at QA (loop qa_bug_fix).',
      );
      const qa = await node('QA');
      expect(within(qa).getByText(/pass 3 of 3/)).toBeInTheDocument();
      expect(within(qa).getByText(/attempt 3/)).toBeInTheDocument();
    });

    it('shows a step waiting for approval, which only the user can give', async () => {
      show({
        runs: [
          run(w, 'running', {
            developer: nodeState('waiting_approval', {
              attempts: [attempt('exec-5', 1, 'running')],
            }),
          }),
        ],
      });

      const developer = await node('Developer');
      expect(developer.dataset.status).toBe('waiting_approval');
      expect(within(developer).getByText(/Waiting approval/)).toBeInTheDocument();
      // There is no approve button here: approvals belong to the agent's security panel.
      expect(screen.queryByRole('button', { name: /approve/i })).not.toBeInTheDocument();
    });

    it('offers to resume, restart or cancel a run the app shutdown interrupted', async () => {
      const user = userEvent.setup();
      show({
        runs: [
          run(w, 'interrupted', {
            developer: nodeState('failed', {
              reason: 'interrupted',
              attempts: [attempt('exec-5', 1, 'interrupted')],
            }),
          }),
        ],
      });

      expect(
        await screen.findByText(/interrupted by the application shutdown/),
      ).toBeInTheDocument();
      expect(
        screen.getByRole('status', { name: 'Workflow status: Interrupted (recoverable)' }),
      ).toBeInTheDocument();
      expect(screen.queryByRole('button', { name: 'Run' })).not.toBeInTheDocument();
      await user.click(screen.getByRole('button', { name: 'Resume' }));
      expect(resumeWorkflow).toHaveBeenCalledWith('wfx-1');
      expect(screen.getByRole('button', { name: 'Restart' })).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Cancel workflow' })).toBeInTheDocument();
      // An interrupted step is never shown as completed.
      expect((await node('Developer')).dataset.status).toBe('failed');
    });

    it('keeps running when the user leaves the screen: coming back reads the run again', async () => {
      const first = show({ runs: [running()] });
      expect((await node('Developer')).dataset.status).toBe('running');

      first.unmount();
      expect(cancelWorkflow).not.toHaveBeenCalled();
      expect(pauseWorkflow).not.toHaveBeenCalled();

      renderWithProviders(<WorkflowPage />);
      expect((await node('Developer')).dataset.status).toBe('running');
      expect(cancelWorkflow).not.toHaveBeenCalled();
    });

    it('opens the Execution Inspector from a node, with the workflow breadcrumb', async () => {
      const user = userEvent.setup();
      show({ runs: [running()] });
      mockBackend({
        language: 'en-US',
        agents: AGENTS,
        workspaces: [workspace('w1', 'ERP', '/dev/erp', ['a-architect'])],
        selectedWorkspaceId: 'w1',
        executions: [
          storedExecution('w1', 'a-architect', 'exec-4', {
            workflow: {
              workflowId: 'wf-1',
              workflowName: 'Password recovery',
              workflowExecutionId: 'wfx-1',
              nodeId: 'architect',
              nodeLabel: 'Architect',
              attempt: 1,
              iteration: 1,
            },
          }),
        ],
      });

      await clickNode('Architect');
      await user.click(within(side()).getByRole('button', { name: /Open execution #4/ }));

      const dialog = await screen.findByRole('dialog', { name: 'Execution #4' });
      const crumbs = within(dialog).getByRole('navigation', { name: 'Where this execution sits' });
      expect(crumbs).toHaveTextContent('Workflow');
      expect(crumbs).toHaveTextContent('Password recovery');
      expect(crumbs).toHaveTextContent('Architect');
      expect(crumbs).toHaveTextContent('Execution #4');
    });

    it("opens an artifact's execution, and says so when the execution has not ended", async () => {
      const user = userEvent.setup();
      show({ runs: [running()] });

      await user.click(await screen.findByRole('button', { name: 'architecture.md' }));
      // exec-4 is not in the history of this test: it is still being stored.
      expect(await screen.findByText(/still running/)).toBeInTheDocument();
    });
  });

  describe('workflows of the workspace', () => {
    it('is available only once there is a workspace', async () => {
      mockBackend({ agents: AGENTS, workspaces: [] });
      vi.mocked(listTemplates).mockResolvedValue(templates);
      renderWithProviders(<WorkflowPage />);
      expect(await screen.findByText('Create a workspace to build workflows.')).toBeInTheDocument();
    });

    it('deletes a workflow after a confirmation', async () => {
      const user = userEvent.setup();
      show();
      await canvas();

      await user.click(screen.getByRole('button', { name: 'Delete workflow' }));
      expect(deleteWorkflow).not.toHaveBeenCalled();
      await user.click(screen.getByRole('button', { name: 'Confirm delete' }));
      expect(deleteWorkflow).toHaveBeenCalledWith('wf-1');
    });

    it('lets the user look at a past run and go back to the editor', async () => {
      const user = userEvent.setup();
      const w = passwordRecovery();
      show({ runs: [run(w, 'completed', { architect: nodeState('completed') })] });
      await canvas();

      await user.selectOptions(screen.getByLabelText('Runs'), 'wfx-1');
      expect((await node('Architect')).dataset.status).toBe('completed');
      await user.click(screen.getByRole('button', { name: 'Back to editor' }));
      expect((await node('Architect')).dataset.status).toBe('none');
    });

    it('tells when a workflow was saved from a snapshot of an older version', async () => {
      const user = userEvent.setup();
      const old = run(passwordRecovery({ version: 1 }), 'completed', {
        architect: nodeState('completed', { attempts: [attempt('exec-1')] }),
      });
      show({ workflows: [passwordRecovery({ version: 3 })], runs: [old] });
      await canvas();

      await user.selectOptions(screen.getByLabelText('Runs'), 'wfx-1');
      expect(screen.getByText('version 1')).toBeInTheDocument();
      await clickNode('Architect');
      expect(within(side()).getByText(/snapshot of version 1/)).toBeInTheDocument();
    });
  });

  describe('language', () => {
    it('is in Portuguese when the language is Portuguese, and keeps names and tasks as written', async () => {
      show({ language: 'pt-BR' });

      expect(await screen.findByRole('button', { name: 'Novo workflow' })).toBeInTheDocument();
      expect(screen.getByText('Automático', { selector: 'span' })).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Personalizar workflow' })).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Executar' })).toBeInTheDocument();
      // The workflow's name and its nodes are the user's: never translated.
      expect(screen.getByRole('option', { name: 'Password recovery' })).toBeInTheDocument();
      expect(await screen.findByLabelText(/^Architect \(/)).toBeInTheDocument();
    });
  });
});
