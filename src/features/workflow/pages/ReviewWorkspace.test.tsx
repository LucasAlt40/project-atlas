import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {
  agent,
  mockBackend,
  renderWithProviders,
  storedExecution,
  workspace,
} from '@/test/fixtures';
import {
  attempt,
  changeSet,
  fileChange,
  handoff,
  liveState,
  nodeState,
  passwordRecovery,
  run,
  templates,
  withCode,
} from '@/test/workflowFixtures';
import type { WorkflowExecutionDto } from '@/lib/tauri/commands';
import {
  applyChanges,
  discardChanges,
  getChanges,
  getDiff,
  getRecovery,
  getRun,
  keepChanges,
  listIdes,
  listRuns,
  listTemplates,
  listWorkflows,
  openInIde,
  selectTemplate,
  subscribeToWorkflowEvents,
  validateWorkflow,
} from '../services/workflowService';
import { getLiveWorkspace } from '../services/liveWorkspaceService';
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

const w = passwordRecovery();

const at = (executionId: string, startedAt: number, outcome: string | null, n = 1) => ({
  ...attempt(executionId, n),
  iteration: n,
  startedAt,
  completedAt: startedAt + 2_000,
  outcome,
});

const finding = {
  severity: 'high',
  category: 'Tests',
  title: 'Reset token never expires',
  file: 'src/auth/password-reset.ts',
  line: 12,
  description: 'The token can be reused.',
  evidence: '',
  recommendation: 'Expire it.',
};

/** Architect, Developer, QA (fails), Bug Fixer, QA (passes). */
const finished = (extra: Partial<WorkflowExecutionDto> = {}) =>
  run(
    w,
    'completed',
    {
      architect: nodeState('completed', { attempts: [at('exec-41', 1_000, 'approved')] }),
      developer: nodeState('completed', { attempts: [at('exec-42', 4_000, 'implemented')] }),
      qa: nodeState('completed', {
        attempts: [at('exec-43', 8_000, 'fail', 1), at('exec-45', 14_000, 'pass', 2)],
      }),
      'bug-fixer': nodeState('completed', { attempts: [at('exec-44', 11_000, 'fixed')] }),
    },
    {
      state: {
        ...run(w, 'completed', {}).state,
        validationResults: [
          {
            nodeId: 'qa',
            executionId: 'exec-43',
            status: 'fail',
            outcome: 'fail',
            summary: 'Found a problem',
            findings: [finding, { ...finding, severity: 'low', title: 'Naming', line: null }],
          },
          {
            nodeId: 'qa',
            executionId: 'exec-45',
            status: 'pass',
            outcome: 'pass',
            summary: '',
            findings: [],
          },
        ],
        decisions: [
          {
            id: 'd1',
            title: 'Use JWT',
            decision: 'Tokens are signed JWTs',
            rationale: 'Already in the codebase',
            sourceNodeId: 'architect',
            createdAt: 1_700_000_000_000,
          },
        ],
        artifacts: [
          {
            id: 'art1',
            type: 'architecture_document',
            name: 'architecture.md',
            producerNodeId: 'architect',
            executionId: 'exec-41',
            path: 'docs/architecture.md',
            summary: 'The reset flow',
            metadata: {},
            createdAt: 1,
          },
        ],
      },
      ...extra,
    },
  );

function show(current: WorkflowExecutionDto) {
  mockBackend({
    language: 'en-US',
    agents: AGENTS,
    workspaces: [workspace('w1', 'ERP', '/dev/erp', ['a-architect'])],
    selectedWorkspaceId: 'w1',
    executions: [
      storedExecution('w1', 'a-developer', 'exec-42', { runtimeId: 'claude', modelId: 'opus' }),
    ],
  });
  vi.mocked(listWorkflows).mockResolvedValue([w]);
  vi.mocked(listRuns).mockResolvedValue([current]);
  vi.mocked(getRun).mockResolvedValue(current);
  vi.mocked(getRecovery).mockResolvedValue(null);
  vi.mocked(listTemplates).mockResolvedValue(templates);
  vi.mocked(selectTemplate).mockResolvedValue('software_feature');
  vi.mocked(validateWorkflow).mockResolvedValue({ valid: true, issues: [] });
  vi.mocked(listIdes).mockResolvedValue([{ id: 'vscode', name: 'Visual Studio Code' }]);
  vi.mocked(subscribeToWorkflowEvents).mockResolvedValue(() => undefined);
  for (const fn of [applyChanges, keepChanges, discardChanges, openInIde, getChanges, getDiff]) {
    vi.mocked(fn).mockReset();
  }
  vi.mocked(getChanges).mockResolvedValue(current.changes);
  vi.mocked(getDiff).mockResolvedValue('');
  return renderWithProviders(<WorkflowPage />);
}

const open = async (user: ReturnType<typeof userEvent.setup>) => {
  await user.selectOptions(await screen.findByLabelText('Runs'), 'wfx-1');
  return screen.findByRole('region', { name: 'Review workspace' });
};

const section = (review: HTMLElement, name: RegExp) =>
  within(review).getByRole('button', { name }).closest('div') as HTMLElement;

describe('the review workspace', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(getLiveWorkspace).mockResolvedValue(null);
  });

  describe('which runs it appears for', () => {
    it.each([
      ['completed', 'Workflow completed'],
      ['failed', 'Workflow failed'],
      ['cancelled', 'Workflow cancelled'],
    ] as const)('reviews a %s workflow', async (status, headline) => {
      const user = userEvent.setup();
      show(withCode({ ...finished(), status }, 'changes_available'));

      const review = await open(user);

      expect(within(review).getAllByText(headline).length).toBeGreaterThan(0);
      expect(within(review).getByText('REVIEW')).toBeInTheDocument();
    });

    it('is not there while the run still works: that is the live workspace', async () => {
      const user = userEvent.setup();
      show(withCode({ ...finished(), status: 'running' }, 'in_progress'));

      await user.selectOptions(await screen.findByLabelText('Runs'), 'wfx-1');
      await screen.findByRole('tablist', { name: 'Run views' });

      expect(screen.queryByRole('region', { name: 'Review workspace' })).not.toBeInTheDocument();
    });

    it('says a failed workflow’s changes may be incomplete, and still lets the user look', async () => {
      const user = userEvent.setup();
      show(
        withCode({ ...finished(), status: 'failed' }, 'changes_available', {
          canApply: false,
          blockReason: 'execution_not_completed',
        }),
      );

      const review = await open(user);

      expect(review).toHaveTextContent('Workflow failed. Changes may be incomplete.');
      expect(within(review).getByRole('button', { name: 'Review diff' })).toBeEnabled();
      expect(within(review).queryByRole('button', { name: 'Apply changes' })).toBeNull();
    });

    it('says what a cancelled workflow shows is what it made until it stopped, and never applies it', async () => {
      const user = userEvent.setup();
      show(
        withCode({ ...finished(), status: 'cancelled' }, 'changes_available', {
          canApply: false,
          blockReason: 'execution_not_completed',
        }),
      );

      const review = await open(user);

      expect(review).toHaveTextContent('what was produced until it stopped');
      expect(within(review).queryByRole('button', { name: 'Apply changes' })).toBeNull();
      expect(applyChanges).not.toHaveBeenCalled();
    });
  });

  describe('summary', () => {
    it('counts steps, agents, files, lines, findings and shows the outcomes agents declared', async () => {
      const user = userEvent.setup();
      show(withCode(finished(), 'changes_available'));

      const review = await open(user);
      const stats = within(review).getByRole('list', { name: 'Summary of the run' });

      expect(stats).toHaveTextContent('5 step(s)');
      expect(stats).toHaveTextContent('4 agent(s)');
      expect(stats).toHaveTextContent('2 file(s) changed');
      expect(stats).toHaveTextContent('+113');
      expect(stats).toHaveTextContent('−4');
      expect(stats).toHaveTextContent('2 finding(s): 1 error(s), 0 warning(s)');
      const outcomes = within(review).getByRole('list', {
        name: 'Outcomes declared by the agents',
      });
      expect(outcomes).toHaveTextContent('Architect');
      expect(outcomes).toHaveTextContent('APPROVED');
      expect(outcomes).toHaveTextContent('IMPLEMENTED');
      expect(outcomes).toHaveTextContent('PASS');
    });
  });

  describe('timeline', () => {
    it('lists each pass in order with its agent, personality, runtime and model, status and outcome', async () => {
      const user = userEvent.setup();
      show(withCode(finished(), 'changes_available'));

      const review = await open(user);
      const steps = within(
        within(review).getByRole('list', { name: 'Agent timeline' }),
      ).getAllByRole('listitem');

      expect(steps.map((s) => s.querySelector('strong')?.textContent)).toEqual([
        'Architect',
        'Developer',
        'QA',
        'Bug Fixer',
        'QA',
      ]);
      const developer = steps[1];
      expect(developer).toHaveTextContent('Developer agent');
      expect(developer).toHaveTextContent('Architect');
      expect(developer).toHaveTextContent('Claude CLI / opus');
      expect(developer).toHaveTextContent('Completed');
      expect(developer).toHaveTextContent('IMPLEMENTED');
      expect(steps[2]).toHaveTextContent('FAIL');
      expect(steps[4]).toHaveTextContent('PASS');
    });
  });

  describe('findings', () => {
    it('shows severity, file, line, the agent that found it and what that agent concluded', async () => {
      const user = userEvent.setup();
      show(withCode(finished(), 'changes_available'));

      const review = await open(user);
      const list = within(review).getByRole('list', { name: 'Findings' });
      const [error, info] = within(list).getAllByRole('listitem');

      expect(error).toHaveTextContent('Error');
      expect(error).toHaveTextContent('Found by QA');
      expect(error).toHaveTextContent('FAIL');
      expect(error).toHaveTextContent('Reset token never expires');
      expect(error).toHaveTextContent('src/auth/password-reset.ts:12');
      expect(error).toHaveTextContent('Expire it.');
      expect(info).toHaveTextContent('Info');
    });

    it('keeps the history from the failing validation to the passing one', async () => {
      const user = userEvent.setup();
      show(withCode(finished(), 'changes_available'));

      const review = await open(user);
      const trail = within(review).getByRole('list', { name: 'From finding to correction' });
      const steps = within(trail).getAllByRole('listitem');

      expect(steps.map((s) => s.querySelector('strong')?.textContent)).toEqual([
        'QA',
        'Bug Fixer',
        'QA',
      ]);
      expect(steps[0]).toHaveTextContent('Reported 2 finding(s)');
      expect(steps[2]).toHaveTextContent('Nothing found');
      expect(within(review).getAllByText('Later validation passed')).toHaveLength(2);
    });

    it('says when nobody reported anything', async () => {
      const user = userEvent.setup();
      const clean = finished();
      show(
        withCode(
          { ...clean, state: { ...clean.state, validationResults: [] } },
          'no_changes',
          {},
          null,
        ),
      );

      const review = await open(user);

      expect(review).toHaveTextContent('No findings');
    });
  });

  describe('handoffs, decisions and artifacts', () => {
    it('shows what git saw next to what the agent claimed', async () => {
      const user = userEvent.setup();
      show(
        withCode(
          finished({
            handoffs: [
              handoff('developer', 'qa', {
                changedFiles: changeSet().files,
                reportedFiles: ['src/auth/auth.controller.ts', 'src/ghost.ts'],
              }),
            ],
          }),
          'changes_available',
        ),
      );

      const review = await open(user);
      await user.click(within(review).getByRole('button', { name: /Handoffs/ }));

      expect(review).toHaveTextContent('The agent also claimed: src/ghost.ts');
      expect(review).toHaveTextContent(
        'Git also detected, not reported by the agent: src/auth/password-reset.ts',
      );
    });

    it('lists decisions with who decided, why and when', async () => {
      const user = userEvent.setup();
      show(withCode(finished(), 'changes_available'));

      const review = await open(user);
      await user.click(within(review).getByRole('button', { name: /Decisions/ }));

      const card = section(review, /Decisions/);
      expect(card).toHaveTextContent('Use JWT');
      expect(card).toHaveTextContent('Tokens are signed JWTs');
      expect(card).toHaveTextContent('Context: Already in the codebase');
      expect(card).toHaveTextContent('Decided by Architect');
    });

    it('lists artifacts by name, kind, origin and location, without a way to open a file', async () => {
      const user = userEvent.setup();
      show(withCode(finished(), 'changes_available'));

      const review = await open(user);
      await user.click(within(review).getByRole('button', { name: /Artifacts/ }));

      const card = section(review, /Artifacts/);
      expect(card).toHaveTextContent('architecture.md');
      expect(card).toHaveTextContent('Architecture document');
      expect(card).toHaveTextContent('Architect');
      expect(card).toHaveTextContent('docs/architecture.md');
      expect(within(card).queryByRole('link')).toBeNull();
    });
  });

  describe('change set', () => {
    it('shows added, modified, deleted, renamed and binary files with their counts', async () => {
      const user = userEvent.setup();
      show(
        withCode(
          finished(),
          'changes_available',
          {},
          changeSet({
            files: [
              fileChange('src/new.ts', 'added'),
              fileChange('src/bar.ts', 'modified'),
              fileChange('src/old.ts', 'deleted'),
              fileChange('src/moved.ts', 'renamed', { oldPath: 'src/was.ts' }),
              fileChange('logo.png', 'modified', {
                binary: true,
                additions: null,
                deletions: null,
              }),
            ],
            filesChanged: 5,
          }),
        ),
      );

      const review = await open(user);
      const set = section(review, /Change set/);

      for (const letter of ['A', 'M', 'D', 'R']) {
        expect(within(set).getAllByText(letter).length).toBeGreaterThan(0);
      }
      expect(set).toHaveTextContent('src/was.ts → src/moved.ts');
      expect(set).toHaveTextContent('binary');
      expect(set).toHaveTextContent('+3 −0');
    });

    it('names who changed a file, from what git measured in each step', async () => {
      const user = userEvent.setup();
      show(
        withCode(
          finished({
            handoffs: [handoff('developer', 'qa', { changedFiles: changeSet().files })],
          }),
          'changes_available',
        ),
      );

      const review = await open(user);

      expect(section(review, /Change set/)).toHaveTextContent('changed by Developer');
    });

    it('opens the diff of the file that was picked, and of all of them', async () => {
      const user = userEvent.setup();
      show(withCode(finished(), 'changes_available'));
      const review = await open(user);

      await user.click(
        within(section(review, /Change set/)).getByRole('button', {
          name: 'Show the diff of src/auth/auth.controller.ts',
        }),
      );

      await screen.findByRole('dialog', { name: 'Review changes' });
      await waitFor(() => {
        expect(getDiff).toHaveBeenLastCalledWith('wfx-1', 'src/auth/auth.controller.ts');
      });
    });

    it('has no changes to apply for an empty change set, and Apply is not offered', async () => {
      const user = userEvent.setup();
      show(withCode(finished(), 'no_changes', {}, null));

      const review = await open(user);

      expect(within(review).getAllByText('No changes to apply.').length).toBeGreaterThan(0);
      expect(within(review).queryByRole('button', { name: 'Apply changes' })).toBeNull();
    });
  });

  describe('review and integration are two different things', () => {
    it('reads ready for review and not applied before Apply', async () => {
      const user = userEvent.setup();
      show(withCode(finished(), 'changes_available'));

      const review = await open(user);
      const result = within(review).getByRole('region', { name: 'Workflow result and code' });

      expect(result).toHaveTextContent('ReviewReady for review');
      expect(result).toHaveTextContent('IntegrationNot applied');
    });

    it('reads reviewed and applied-uncommitted after Apply, never integrated', async () => {
      const user = userEvent.setup();
      show(withCode(finished(), 'integrated', { canUndo: true }));

      const review = await open(user);
      const result = within(review).getByRole('region', { name: 'Workflow result and code' });

      expect(result).toHaveTextContent('ReviewReviewed');
      expect(result).toHaveTextContent('IntegrationApplied to the working tree — uncommitted');
      expect(result).toHaveTextContent('Changes applied.');
      expect(result).toHaveTextContent('No commit was created.');
      expect(result).toHaveTextContent('HEAD unchanged · changes uncommitted.');
      expect(result).not.toHaveTextContent(/\bIntegrated\b/);
    });

    it('does not claim HEAD is unchanged when the project has moved on', async () => {
      const user = userEvent.setup();
      show(withCode(finished(), 'integrated', { canUndo: false }));

      const review = await open(user);

      expect(review).not.toHaveTextContent('HEAD unchanged');
      expect(review).toHaveTextContent('can no longer tell whether they are still uncommitted');
    });

    it('says the worktree is where the review happens and the project is where Apply lands', async () => {
      const user = userEvent.setup();
      show(withCode(finished(), 'changes_available'));

      const review = await open(user);

      expect(review).toHaveTextContent('in the workflow’s isolated worktree');
      expect(review).toHaveTextContent(
        'Your project does not have these changes until you apply them',
      );
    });
  });

  describe('apply', () => {
    it('asks first, says what it will not do, and does nothing until confirmed', async () => {
      const user = userEvent.setup();
      show(withCode(finished(), 'changes_available'));
      const review = await open(user);

      await user.click(within(review).getByRole('button', { name: 'Apply changes' }));

      const dialog = await screen.findByRole('dialog', { name: 'Apply changes?' });
      expect(dialog).toHaveTextContent('(2 file(s))');
      expect(dialog).toHaveTextContent('create a commit');
      expect(dialog).toHaveTextContent('push to a remote');
      expect(dialog).toHaveTextContent('merge branches');
      expect(dialog).toHaveTextContent('uncommitted changes');
      expect(applyChanges).not.toHaveBeenCalled();
    });

    it('does not apply when the user cancels', async () => {
      const user = userEvent.setup();
      show(withCode(finished(), 'changes_available'));
      const review = await open(user);
      await user.click(within(review).getByRole('button', { name: 'Apply changes' }));

      await user.click(
        within(await screen.findByRole('dialog', { name: 'Apply changes?' })).getByRole('button', {
          name: 'Cancel',
        }),
      );

      expect(screen.queryByRole('dialog', { name: 'Apply changes?' })).toBeNull();
      expect(applyChanges).not.toHaveBeenCalled();
    });

    it('applies once confirmed, and shows the post-apply state the core returned', async () => {
      const user = userEvent.setup();
      show(withCode(finished(), 'changes_available'));
      vi.mocked(applyChanges).mockResolvedValue(
        withCode(finished(), 'integrated', { canUndo: true }),
      );
      const review = await open(user);
      await user.click(within(review).getByRole('button', { name: 'Apply changes' }));

      await user.click(screen.getByRole('button', { name: 'Apply to working tree' }));

      expect(applyChanges).toHaveBeenCalledTimes(1);
      expect(applyChanges).toHaveBeenCalledWith('wfx-1');
      await waitFor(() => {
        expect(review).toHaveTextContent('Changes applied.');
      });
      expect(keepChanges).not.toHaveBeenCalled();
      expect(discardChanges).not.toHaveBeenCalled();
    });

    it('presents a blocked apply with the conflicting files and what to do', async () => {
      const user = userEvent.setup();
      show(withCode(finished(), 'changes_available'));
      vi.mocked(applyChanges).mockResolvedValue(
        withCode(finished(), 'conflicts', {
          conflicts: ['src/foo.ts'],
          blockReason: 'conflict',
          canApply: true,
        }),
      );
      const review = await open(user);
      await user.click(within(review).getByRole('button', { name: 'Apply changes' }));
      await user.click(screen.getByRole('button', { name: 'Apply to working tree' }));

      await waitFor(() => {
        expect(review).toHaveTextContent('Apply blocked.');
      });
      expect(review).toHaveTextContent('Conflicting files:');
      expect(within(review).getByText('src/foo.ts')).toBeInTheDocument();
      expect(review).toHaveTextContent('Resolve it yourself');
      expect(review).toHaveTextContent('Not applied — apply blocked');
      expect(review).toHaveTextContent('Nothing in your project changed.');
    });

    it('lists the conflicts of the last attempt in the confirmation, and still blocks nothing by itself', async () => {
      const user = userEvent.setup();
      show(
        withCode(finished(), 'conflicts', {
          conflicts: ['src/foo.ts'],
          blockReason: 'conflict',
          canApply: true,
        }),
      );
      const review = await open(user);

      await user.click(within(review).getByRole('button', { name: 'Apply changes' }));

      const dialog = await screen.findByRole('dialog', { name: 'Apply changes?' });
      expect(dialog).toHaveTextContent('also changed in your project');
      expect(within(dialog).getByText('src/foo.ts')).toBeInTheDocument();
      expect(dialog).toHaveTextContent('nothing is overwritten');
    });
  });

  describe('when the worktree is gone', () => {
    it('says so, keeps showing what was saved, and offers only what still makes sense', async () => {
      const user = userEvent.setup();
      vi.mocked(getLiveWorkspace).mockResolvedValue(
        liveState({ phase: 'ended', availability: 'missing', files: [fileChange('src/auth.ts')] }),
      );
      show(withCode(finished(), 'changes_available'));

      const review = await open(user);

      expect(await within(review).findByText(/no longer available/)).toBeInTheDocument();
      const result = within(review).getByRole('region', { name: 'Workflow result and code' });
      expect(within(result).queryByRole('button', { name: 'Apply changes' })).toBeNull();
      expect(within(result).queryByRole('button', { name: 'Open in IDE' })).toBeNull();
      expect(within(result).queryByRole('button', { name: 'Keep isolated' })).toBeNull();
      expect(within(result).getByRole('button', { name: 'Review diff' })).toBeEnabled();
      // The saved change set is still there.
      expect(section(review, /Change set/)).toHaveTextContent('src/auth/auth.controller.ts');
    });
  });
});
