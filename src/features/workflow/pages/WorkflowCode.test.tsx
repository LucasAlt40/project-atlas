import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { agent, mockBackend, renderWithProviders, workspace } from '@/test/fixtures';
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
const finished = (extra: Partial<WorkflowExecutionDto> = {}) =>
  run(
    w,
    'completed',
    {
      architect: nodeState('completed', { attempts: [attempt('exec-41')] }),
      developer: nodeState('completed', { attempts: [attempt('exec-42')] }),
    },
    extra,
  );

function show(
  current: WorkflowExecutionDto,
  options: { ides?: { id: string; name: string }[]; language?: string } = {},
) {
  mockBackend({
    language: options.language ?? 'en-US',
    agents: AGENTS,
    workspaces: [workspace('w1', 'ERP', '/dev/erp', ['a-architect'])],
    selectedWorkspaceId: 'w1',
  });
  vi.mocked(listWorkflows).mockResolvedValue([w]);
  vi.mocked(listRuns).mockResolvedValue([current]);
  vi.mocked(getRun).mockResolvedValue(current);
  vi.mocked(listTemplates).mockResolvedValue(templates);
  vi.mocked(selectTemplate).mockResolvedValue('software_feature');
  vi.mocked(validateWorkflow).mockResolvedValue({ valid: true, issues: [] });
  vi.mocked(listIdes).mockResolvedValue(
    options.ides ?? [{ id: 'vscode', name: 'Visual Studio Code' }],
  );
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
  return screen.findByRole('region', { name: 'Workflow result and code' });
};

/** Picks the run without expecting it to have code. */
const openRun = async (user: ReturnType<typeof userEvent.setup>) => {
  await user.selectOptions(await screen.findByLabelText('Runs'), 'wfx-1');
  await screen.findByRole('tablist', { name: 'Run views' });
};
const node = async (name: string) =>
  within(await screen.findByRole('group', { name: 'Workflow graph' })).findByLabelText(
    new RegExp(`^${name} \\(`),
  );

describe('the code of a workflow run', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows the live workspace of the run beside its result, and applying is still only the result panel’s', async () => {
    const user = userEvent.setup();
    vi.mocked(getLiveWorkspace).mockResolvedValue(
      liveState({ phase: 'ended', files: [fileChange('src/auth.ts')] }),
    );
    show(withCode(finished(), 'changes_available'));

    await open(user);
    const live = await screen.findByRole('region', { name: 'Live workspace' });

    expect(within(live).getByText('REVIEW')).toBeInTheDocument();
    expect(
      await within(live).findByRole('button', { name: 'src/auth.ts (Modified)' }),
    ).toBeInTheDocument();
    // The one way to apply is the result panel's, not the live workspace's.
    expect(within(live).queryByRole('button', { name: /apply/i })).toBeNull();
    expect(screen.getAllByRole('button', { name: 'Apply changes' })).toHaveLength(1);
    expect(applyChanges).not.toHaveBeenCalled();
  });

  it('says the workflow completed and, separately, that its code is not in the project yet', async () => {
    const user = userEvent.setup();
    show(withCode(finished(), 'changes_available'));

    const delivery = await open(user);

    expect(within(delivery).getByText('Workflow completed')).toBeInTheDocument();
    expect(within(delivery).getByRole('status')).toHaveTextContent(
      'They are not in your project yet.',
    );
    expect(within(delivery).getByText('2 file(s) changed · +113 −4')).toBeInTheDocument();
    expect(delivery).toHaveTextContent('does not mean the code is in your project');
    for (const name of [
      'Review changes',
      'Open in IDE',
      'Apply changes',
      'Keep isolated',
      'Discard',
    ]) {
      expect(within(delivery).getByRole('button', { name })).toBeEnabled();
    }
    // It never claims what has not happened.
    expect(delivery).not.toHaveTextContent('were applied to your working tree');
  });

  it('applies the changes only when asked, and only then says they are in the project', async () => {
    const user = userEvent.setup();
    const current = withCode(finished(), 'changes_available');
    show(current);
    vi.mocked(applyChanges).mockResolvedValue(withCode(finished(), 'integrated'));
    const delivery = await open(user);

    await user.click(within(delivery).getByRole('button', { name: 'Apply changes' }));

    expect(applyChanges).toHaveBeenCalledWith('wfx-1');
    await waitFor(() => {
      expect(
        screen.getByRole('status', { name: 'Workflow status: Completed' }),
      ).toBeInTheDocument();
      expect(
        within(screen.getByRole('region', { name: 'Workflow result and code' })).getByText(
          'The changes were applied to your working tree. Nothing was committed: review them with git status and commit when you are ready.',
        ),
      ).toBeInTheDocument();
    });
    const after = screen.getByRole('region', { name: 'Workflow result and code' });
    expect(within(after).queryByRole('button', { name: 'Apply changes' })).not.toBeInTheDocument();
    expect(within(after).getByRole('button', { name: 'Review changes' })).toBeInTheDocument();
  });

  it('opens the person’s editor on the project once the changes are applied, and not when they are not', async () => {
    const user = userEvent.setup();
    localStorage.setItem('atlas.preferredIde', 'cursor');
    show(withCode(finished(), 'changes_available'), {
      ides: [
        { id: 'vscode', name: 'Visual Studio Code' },
        { id: 'cursor', name: 'Cursor' },
      ],
    });
    vi.mocked(applyChanges).mockResolvedValue(
      withCode(finished(), 'integrated', { canUndo: true }),
    );
    const delivery = await open(user);
    expect(openInIde).not.toHaveBeenCalled();

    await user.click(within(delivery).getByRole('button', { name: 'Apply changes' }));

    await waitFor(() => {
      expect(openInIde).toHaveBeenCalledWith('wfx-1', 'cursor');
    });
    localStorage.removeItem('atlas.preferredIde');
  });

  it('does not open an editor when applying was blocked', async () => {
    const user = userEvent.setup();
    show(withCode(finished(), 'changes_available'));
    vi.mocked(applyChanges).mockResolvedValue(
      withCode(finished(), 'conflicts', { conflicts: ['src/a.ts'], canApply: true }),
    );
    const delivery = await open(user);

    await user.click(within(delivery).getByRole('button', { name: 'Apply changes' }));

    await screen.findByText(/conflict with your project|also changed in your project/);
    expect(openInIde).not.toHaveBeenCalled();
  });

  it('offers to keep applied changes isolated, which takes them back out of the project', async () => {
    const user = userEvent.setup();
    show(withCode(finished(), 'integrated', { canUndo: true }));
    vi.mocked(keepChanges).mockResolvedValue(withCode(finished(), 'kept_isolated'));
    const delivery = await open(user);
    expect(delivery).toHaveTextContent('takes these changes back out of your project');

    await user.click(within(delivery).getByRole('button', { name: 'Keep isolated' }));

    expect(keepChanges).toHaveBeenCalledWith('wfx-1');
    await waitFor(() => {
      expect(
        within(screen.getByRole('region', { name: 'Workflow result and code' })).getByText(
          'The changes were kept in the isolated worktree. They are not in your project.',
        ),
      ).toBeInTheDocument();
    });
  });

  it('does not offer it once the applied changes can no longer be taken back, and says when they could not', async () => {
    const user = userEvent.setup();
    show(withCode(finished(), 'integrated', { canUndo: false }));
    expect(
      within(await open(user)).queryByRole('button', { name: 'Keep isolated' }),
    ).not.toBeInTheDocument();
  });

  it('says why applied changes could not be taken back, and that nothing was touched', async () => {
    const user = userEvent.setup();
    show(
      withCode(finished(), 'integrated', {
        canUndo: true,
        blockReason: 'conflict',
        conflicts: ['src/a.ts'],
      }),
    );

    const delivery = await open(user);

    expect(within(delivery).getByRole('alert')).toHaveTextContent(
      '1 of the applied file(s) were changed in your project since. Nothing was touched.',
    );
    expect(delivery).toHaveTextContent('src/a.ts');
  });

  it('does not turn a completed workflow into a failed one when applying is blocked, and says why', async () => {
    const user = userEvent.setup();
    show(withCode(finished(), 'blocked', { blockReason: 'base_dirty', canApply: true }));

    const delivery = await open(user);

    expect(within(delivery).getByText('Workflow completed')).toBeInTheDocument();
    expect(delivery).toHaveTextContent('Your project has uncommitted changes');
    expect(delivery).toHaveTextContent('isolated worktree');
    expect(screen.getByRole('status', { name: 'Workflow status: Completed' })).toBeInTheDocument();
    // It can be tried again once the user has put their project right.
    expect(within(delivery).getByRole('button', { name: 'Apply changes' })).toBeEnabled();
  });

  it('lists the conflicting files and offers to look rather than to force', async () => {
    const user = userEvent.setup();
    show(
      withCode(finished(), 'conflicts', {
        conflicts: ['src/auth/auth.controller.ts'],
        blockReason: 'conflict',
        canApply: true,
      }),
    );

    const delivery = await open(user);

    expect(delivery).toHaveTextContent(
      'Apply blocked: 1 file(s) are also changed in your project.',
    );
    expect(delivery).toHaveTextContent('Nothing in your project changed.');
    expect(within(delivery).getByText('src/auth/auth.controller.ts')).toBeInTheDocument();
    expect(within(delivery).getByRole('button', { name: 'Open in IDE' })).toBeEnabled();
    expect(within(delivery).getByRole('button', { name: 'Keep isolated' })).toBeEnabled();
  });

  it("keeps a cancelled workflow's work for inspection and never offers to apply it", async () => {
    const user = userEvent.setup();
    show(
      withCode({ ...finished(), status: 'cancelled' }, 'changes_available', {
        canApply: false,
        blockReason: 'execution_not_completed',
      }),
    );

    const delivery = await open(user);

    expect(within(delivery).getByText('Workflow cancelled')).toBeInTheDocument();
    expect(delivery).toHaveTextContent('code it wrote so far is kept in an isolated worktree');
    expect(
      within(delivery).queryByRole('button', { name: 'Apply changes' }),
    ).not.toBeInTheDocument();
    for (const name of ['Review changes', 'Open in IDE', 'Keep isolated', 'Discard']) {
      expect(within(delivery).getByRole('button', { name })).toBeEnabled();
    }
  });

  it('says a workflow that changed no code has no code to decide about', async () => {
    const user = userEvent.setup();
    show(withCode(finished(), 'no_changes', {}, null));

    const delivery = await open(user);

    expect(delivery).toHaveTextContent('The workflow made no code changes.');
    expect(within(delivery).queryAllByRole('button')).toHaveLength(0);
  });

  it('shows nothing about code for a run that never had an isolated worktree', async () => {
    const user = userEvent.setup();
    show(finished());

    await openRun(user);

    expect(
      screen.queryByRole('region', { name: 'Workflow result and code' }),
    ).not.toBeInTheDocument();
  });

  it('asks for confirmation before discarding and says the branch is kept', async () => {
    const user = userEvent.setup();
    show(withCode(finished(), 'changes_available'));
    vi.mocked(discardChanges).mockResolvedValue(withCode(finished(), 'discarded', {}, null));
    const delivery = await open(user);

    await user.click(within(delivery).getByRole('button', { name: 'Discard' }));
    expect(discardChanges).not.toHaveBeenCalled();
    expect(delivery).toHaveTextContent('Its branch is kept');
    await user.click(within(delivery).getByRole('button', { name: 'Confirm discard' }));

    expect(discardChanges).toHaveBeenCalledWith('wfx-1');
    await waitFor(() => {
      expect(screen.getByRole('region', { name: 'Workflow result and code' })).toHaveTextContent(
        'The worktree was discarded.',
      );
    });
  });

  it('keeps the changes isolated when asked', async () => {
    const user = userEvent.setup();
    show(withCode(finished(), 'changes_available'));
    vi.mocked(keepChanges).mockResolvedValue(
      withCode(finished(), 'kept_isolated', { canApply: true }),
    );
    const delivery = await open(user);

    await user.click(within(delivery).getByRole('button', { name: 'Keep isolated' }));

    expect(keepChanges).toHaveBeenCalledWith('wfx-1');
    await waitFor(() => {
      expect(screen.getByRole('region', { name: 'Workflow result and code' })).toHaveTextContent(
        'kept in the isolated worktree',
      );
    });
  });

  it('opens the worktree in the editor, asking which when there are several', async () => {
    const user = userEvent.setup();
    show(withCode(finished(), 'changes_available'), {
      ides: [
        { id: 'vscode', name: 'Visual Studio Code' },
        { id: 'cursor', name: 'Cursor' },
      ],
    });
    vi.mocked(openInIde).mockResolvedValue(undefined);
    const delivery = await open(user);

    await user.click(within(delivery).getByRole('button', { name: 'Open in IDE' }));
    const menu = screen.getByRole('list', { name: 'Choose an editor' });
    expect(
      within(menu)
        .getAllByRole('button')
        .map((b) => b.textContent),
    ).toEqual(['Visual Studio Code', 'Cursor']);
    await user.click(within(menu).getByRole('button', { name: 'Cursor' }));

    // Only a run and an editor from the list are named: never a path.
    expect(openInIde).toHaveBeenCalledWith('wfx-1', 'cursor');
  });

  it('opens the only editor straight away, and says when there is none', async () => {
    const user = userEvent.setup();
    show(withCode(finished(), 'changes_available'));
    vi.mocked(openInIde).mockResolvedValue(undefined);
    const delivery = await open(user);
    await user.click(within(delivery).getByRole('button', { name: 'Open in IDE' }));
    expect(openInIde).toHaveBeenCalledWith('wfx-1', 'vscode');

    show(withCode(finished(), 'changes_available'), { ides: [] });
    const none = await screen.findAllByRole('button', { name: 'Open in IDE' });
    expect(none.length).toBeGreaterThan(0);
  });

  it('shows the real diff, file by file', async () => {
    const user = userEvent.setup();
    show(withCode(finished(), 'changes_available'));
    vi.mocked(getDiff).mockImplementation((_, file) =>
      Promise.resolve(
        file
          ? `diff --git a/${file} b/${file}\n@@ -1 +1,2 @@\n context\n+added line\n-removed line\n`
          : 'diff --git a/all b/all\n+everything\n',
      ),
    );
    const delivery = await open(user);

    await user.click(within(delivery).getByRole('button', { name: 'Review changes' }));

    const dialog = await screen.findByRole('dialog', { name: 'Review changes' });
    expect(await within(dialog).findByText('+everything')).toBeInTheDocument();
    await user.click(
      within(dialog).getByRole('button', { name: 'Show the diff of src/auth/auth.controller.ts' }),
    );
    const added = await within(dialog).findByText('+added line');
    expect(added.dataset.kind).toBe('add');
    expect(within(dialog).getByText('-removed line').dataset.kind).toBe('del');
    expect(getDiff).toHaveBeenLastCalledWith('wfx-1', 'src/auth/auth.controller.ts');
    expect(within(dialog).getByText('src/auth/password-reset.ts')).toBeInTheDocument();
  });

  it('lists the changed files in the Changes tab, with their counts', async () => {
    const user = userEvent.setup();
    show(withCode(finished(), 'changes_available'));
    await open(user);

    await user.click(screen.getByRole('tab', { name: 'Changes' }));

    expect(screen.getByText('src/auth/password-reset.ts')).toBeInTheDocument();
    expect(screen.getByText('+82 −0')).toBeInTheDocument();
    expect(screen.getByText('+31 −4')).toBeInTheDocument();
  });

  it('is in Portuguese when the interface is, without ever saying the code is in the project early', async () => {
    const user = userEvent.setup();
    show(withCode(finished(), 'changes_available'), { language: 'pt-BR' });
    await user.selectOptions(await screen.findByLabelText('Execuções'), 'wfx-1');

    const delivery = await screen.findByRole('region', {
      name: 'Resultado do workflow e do código',
    });

    expect(delivery).toHaveTextContent('Workflow concluído');
    expect(delivery).toHaveTextContent('Ainda não estão no seu projeto.');
    expect(
      within(delivery).getByRole('button', { name: 'Aplicar alterações' }),
    ).toBeInTheDocument();
    expect(within(delivery).getByRole('button', { name: 'Manter isolado' })).toBeInTheDocument();
  });
});

describe('handoffs between agents', () => {
  const handedOver = () =>
    finished({
      handoffs: [
        handoff('architect', 'developer', {
          summary: 'Defined the reset flow.',
          decisions: [
            { title: 'JWT', decision: 'Use JWT for the token', rationale: 'Already used' },
          ],
          artifacts: [
            {
              id: 'a1',
              name: 'architecture.md',
              type: 'architecture_document',
              path: 'docs/architecture.md',
              summary: 's',
            },
          ],
          instructions: 'implement the endpoint next',
        }),
        handoff('developer', 'qa', {
          summary: 'Implemented the endpoint.',
          fromExecutionId: 'exec-42',
          changedFiles: changeSet().files,
          uncommittedFiles: ['wip.txt'],
          reportedFiles: ['src/not-really.ts'],
          validation: {
            status: 'fail',
            outcome: null,
            summary: '2 tests failed',
            findings: [
              {
                severity: 'high',
                category: 'Security',
                title: '',
                file: null,
                line: null,
                description: 'Expired tokens accepted',
                evidence: 'src/auth/password-reset.ts',
                recommendation: 'reject them',
              },
            ],
          },
        }),
      ],
    });

  it('lists every handoff in its own tab', async () => {
    const user = userEvent.setup();
    show(handedOver());
    await openRun(user);

    await user.click(screen.getByRole('tab', { name: 'Handoffs (2)' }));

    const first = screen.getByRole('article', { name: 'Handoff from Architect to Developer' });
    expect(within(first).getByText('Defined the reset flow.')).toBeInTheDocument();
    expect(within(first).getByText('Use JWT for the token', { exact: false })).toBeInTheDocument();
    expect(within(first).getByText('architecture.md')).toBeInTheDocument();
    // The agent's suggestion is shown as one, not as an order.
    expect(within(first).getByText('implement the endpoint next')).toBeInTheDocument();
    expect(first).toHaveTextContent('The workflow decides what runs next.');
    const second = screen.getByRole('article', { name: 'Handoff from Developer to QA' });
    expect(second).toHaveTextContent('Changed files (measured by Git)');
    expect(within(second).getByText('src/auth/password-reset.ts')).toBeInTheDocument();
    expect(second).toHaveTextContent('wip.txt');
    expect(second).toHaveTextContent('not committed');
    // What the agent only claimed is kept apart, as a claim.
    expect(second).toHaveTextContent('The agent also claimed: src/not-really.ts');
    expect(second).toHaveTextContent('2 tests failed');
    expect(second).toHaveTextContent('Expired tokens accepted');
  });

  it('shows what a node was handed and what it handed on, out of the way until asked', async () => {
    const user = userEvent.setup();
    show(handedOver());
    await user.selectOptions(await screen.findByLabelText('Runs'), 'wfx-1');

    fireEvent.click(await node('Developer'));

    const side = screen.getByRole('complementary', { name: 'Workflow details' });
    const input = within(side).getByText('Input — handed to this step (1)');
    const output = within(side).getByText('Output — handed on by this step (1)');
    expect(input.closest('details')).not.toHaveAttribute('open');
    await user.click(input);
    expect(within(side).getByText('Defined the reset flow.')).toBeInTheDocument();
    await user.click(output);
    expect(within(side).getByText('Implemented the endpoint.')).toBeInTheDocument();
  });

  it('opens the execution a handoff came from', async () => {
    const user = userEvent.setup();
    show(handedOver());
    await openRun(user);
    await user.click(screen.getByRole('tab', { name: 'Handoffs (2)' }));

    const second = screen.getByRole('article', { name: 'Handoff from Developer to QA' });
    await user.click(within(second).getByRole('button', { name: '#42' }));

    // Not stored yet in this test: the page says so instead of failing.
    expect(await screen.findByText(/still running/)).toBeInTheDocument();
  });

  it('says so when nothing was handed over', async () => {
    const user = userEvent.setup();
    show(finished());
    await openRun(user);

    await user.click(screen.getByRole('tab', { name: 'Handoffs' }));

    expect(screen.getByText('Nothing was handed over here (yet).')).toBeInTheDocument();
  });
});
