import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { CommandError, type SpecChangedDto, type SpecViewDto } from '@/lib/tauri/commands';
import { subscribeToExecutionEvents } from '@/features/workspace/services/workspaceService';
import { agent, mockBackend, navigate, renderWithProviders, workspace } from '@/test/fixtures';
import type { spec as specDto } from '@/test/specFixtures';
import { allApproved, artifact, file, issue, summary, view } from '@/test/specFixtures';
import {
  answerFindings,
  archiveSpec,
  deleteSpec,
  answerQuestion,
  approveSpec,
  archiveWithCli,
  checkWithCli,
  getCliStatus,
  cancelRun,
  createSpec,
  decideArtifact,
  generateArtifact,
  getSpec,
  importSpec,
  keepArtifact,
  listImportable,
  listSpecs,
  publishSpec,
  raiseBudget,
  resolveFinding,
  runReview,
  startSpec,
  subscribeToSpecChanges,
  validateSpec,
} from '../services/specService';
import { listMcpConnections } from '@/features/workspace/services/mcpService';
import { SpecsPage } from './SpecsPage';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');
vi.mock('@/features/workspace/services/mcpService');
vi.mock('../services/specService');

function first<T>(items: T[]): T {
  const item = items[0];
  if (item === undefined) throw new Error('nothing found');
  return item;
}

let emit: (change: SpecChangedDto) => void = () => undefined;

function backend() {
  mockBackend({
    selectedWorkspaceId: 'ws1',
    agents: [agent('a1', 'Architect')],
    workspaces: [workspace('ws1', 'Atlas', '/atlas', ['a1'])],
  });
  vi.mocked(subscribeToSpecChanges).mockImplementation((handler) => {
    emit = handler;
    return Promise.resolve(() => undefined);
  });
  vi.mocked(subscribeToExecutionEvents).mockResolvedValue(() => undefined);
}

/** Opens the page on one spec, as the person does: from the list. */
async function open(current: SpecViewDto) {
  backend();
  vi.mocked(listSpecs).mockResolvedValue([
    summary({ id: current.spec.id, title: current.spec.title }),
  ]);
  vi.mocked(getSpec).mockResolvedValue(current);
  const user = userEvent.setup();
  renderWithProviders(<SpecsPage />);
  await user.click(await screen.findByRole('button', { name: current.spec.title }));
  await screen.findByRole('heading', { level: 2, name: current.spec.title });
  return user;
}

const proposalWritten = () =>
  view('awaiting_review', {
    artifacts: [
      artifact('proposal', { review: 'awaiting_review' }),
      artifact('specs'),
      artifact('design'),
      artifact('tasks'),
    ],
  });

beforeEach(() => {
  vi.clearAllMocks();
  // Answers not yet sent are kept in the browser: one test's must not reach the next.
  localStorage.clear();
  vi.mocked(listMcpConnections).mockResolvedValue({ connections: [], grants: [] });
});

/** The panels of the final review fold down to their title and state; open the one a test needs. */
async function openPanel(title: string) {
  const toggle = await screen.findByRole('button', { name: title });
  if (toggle.getAttribute('aria-expanded') === 'false') await userEvent.click(toggle);
}

describe('the Specs overview', () => {
  it('starts with the two ways to begin, and runs nothing', async () => {
    backend();
    vi.mocked(listSpecs).mockResolvedValue([]);
    renderWithProviders(<SpecsPage />);

    expect(await screen.findByText('No specifications yet')).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: 'New specification' }).length).toBeGreaterThan(0);
    expect(screen.getAllByRole('button', { name: 'Import specs' }).length).toBeGreaterThan(0);
    expect(startSpec).not.toHaveBeenCalled();
    expect(generateArtifact).not.toHaveBeenCalled();
  });

  it('shows where each spec is, how far, and what waits for the person', async () => {
    backend();
    vi.mocked(listSpecs).mockResolvedValue([
      summary({
        id: 's1',
        title: 'Dark mode',
        status: 'awaiting_review',
        approved: 1,
        pendingQuestions: 1,
      }),
      summary({
        id: 's2',
        title: 'Export data',
        changeId: 'export-data',
        status: 'ready_for_implementation',
        approved: 4,
        currentArtifact: null,
      }),
      summary({ id: 's3', title: 'Old idea', changeId: 'old-idea', status: 'archived' }),
    ]);
    const user = userEvent.setup();
    renderWithProviders(<SpecsPage />);

    const dark = await screen.findByRole('button', { name: 'Dark mode' });
    expect(within(dark).getByText('Awaiting your review')).toBeInTheDocument();
    expect(within(dark).getByText('1/4 approved')).toBeInTheDocument();
    expect(within(dark).getByText('Now: Requirements')).toBeInTheDocument();
    expect(within(dark).getByText('1 question(s) for you')).toBeInTheDocument();
    // The counters are the counts of the group tabs.
    expect(screen.getByRole('button', { name: 'Waiting for you (1)' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Ready (1)' })).toBeInTheDocument();
    // An archived spec is out of the way until asked for.
    expect(screen.queryByRole('button', { name: 'Old idea' })).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: /^Ready/ }));
    expect(screen.queryByRole('button', { name: 'Dark mode' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Export data' })).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: /^Archived/ }));
    expect(screen.getByRole('button', { name: 'Old idea' })).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: /^All/ }));
    await user.type(screen.getByRole('searchbox'), 'export');
    expect(screen.queryByRole('button', { name: 'Dark mode' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Export data' })).toBeInTheDocument();
    await user.clear(screen.getByRole('searchbox'));
    await user.type(screen.getByRole('searchbox'), 'zzz');
    expect(screen.getByText('No spec matches this filter.')).toBeInTheDocument();
  });

  it('archives a published spec and deletes one that built nothing, from the row menu', async () => {
    backend();
    vi.mocked(listSpecs).mockResolvedValue([
      summary({ id: 's1', title: 'Published one', status: 'published', implementations: 2 }),
      summary({ id: 's2', title: 'Draft one', changeId: 'draft-one', status: 'draft' }),
    ]);
    vi.mocked(archiveSpec).mockResolvedValue(view('published'));
    vi.mocked(deleteSpec).mockResolvedValue(undefined);
    const user = userEvent.setup();
    renderWithProviders(<SpecsPage />);

    // A spec that was implemented cannot be deleted, but can be archived.
    await user.click(await screen.findByRole('button', { name: 'More actions for Published one' }));
    expect(screen.queryByRole('menuitem', { name: 'Delete' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('menuitem', { name: 'Archive spec' }));
    expect(archiveSpec).toHaveBeenCalledWith('s1');

    // A draft cannot be archived, only deleted, and only after confirming.
    await user.click(screen.getByRole('button', { name: 'More actions for Draft one' }));
    expect(screen.queryByRole('menuitem', { name: 'Archive spec' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('menuitem', { name: 'Delete' }));
    expect(deleteSpec).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Confirm delete' }));
    expect(deleteSpec).toHaveBeenCalledWith('s2');
  });

  it('says the core could not be read instead of showing an empty list', async () => {
    backend();
    vi.mocked(listSpecs).mockRejectedValue(
      new CommandError('list_specs', { code: 'storage_failed', params: {}, detail: null }),
    );
    renderWithProviders(<SpecsPage />);

    expect(await screen.findByRole('alert')).toBeInTheDocument();
  });
});

describe('creating a spec', () => {
  it('needs a title and a request, records them, and starts nothing until confirmed', async () => {
    backend();
    vi.mocked(listSpecs).mockResolvedValue([]);
    const draft = view('draft', { started: false });
    vi.mocked(createSpec).mockResolvedValue(draft);
    vi.mocked(getSpec).mockResolvedValue(draft);
    const user = userEvent.setup();
    renderWithProviders(<SpecsPage />);

    await user.click(first(await screen.findAllByRole('button', { name: 'New specification' })));
    const dialog = await screen.findByRole('dialog', { name: 'New specification' });
    const create = within(dialog).getByRole('button', { name: 'Create' });
    expect(create).toBeDisabled();
    expect(within(dialog).getByText(/No agent runs until you start it/)).toBeInTheDocument();

    await user.type(within(dialog).getByLabelText('Title'), 'Dark mode');
    expect(create).toBeDisabled();
    await user.type(within(dialog).getByLabelText(/What do you want built/), 'Add a dark mode');
    expect(create).toBeEnabled();
    expect(within(dialog).getByLabelText('Agent that writes')).toHaveValue('a1');
    await user.click(create);

    expect(createSpec).toHaveBeenCalledWith({
      workspaceId: 'ws1',
      title: 'Dark mode',
      request: 'Add a dark mode',
      context: '',
      authorAgentId: 'a1',
      reviewerAgentId: 'a1',
      integrations: [],
    });
    expect(await screen.findByRole('heading', { level: 2, name: 'Dark mode' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Start specification' })).toBeEnabled();
    expect(screen.getByText(/Nothing has run yet/)).toBeInTheDocument();
    expect(startSpec).not.toHaveBeenCalled();

    vi.mocked(startSpec).mockResolvedValue({ view: view('generating'), launchError: null });
    await user.click(screen.getByRole('button', { name: 'Start specification' }));
    expect(startSpec).toHaveBeenCalledWith('spec-1');
  });

  it('asks for context that fits the description and the project, and sends the answers with the request', async () => {
    backend();
    vi.mocked(listSpecs).mockResolvedValue([]);
    const draft = view('draft', { started: false });
    vi.mocked(createSpec).mockResolvedValue(draft);
    vi.mocked(getSpec).mockResolvedValue(draft);
    const user = userEvent.setup();
    renderWithProviders(<SpecsPage />);
    await user.click(first(await screen.findAllByRole('button', { name: 'New specification' })));
    const dialog = await screen.findByRole('dialog', { name: 'New specification' });

    expect(within(dialog).getByText(/questions that fit it appear here/)).toBeInTheDocument();
    await user.type(within(dialog).getByLabelText('Title'), 'Cancel plan');
    await user.type(
      within(dialog).getByLabelText(/What do you want built/),
      'Delete the account rows and cancel the subscription in the payment gateway',
    );

    // The project here is an Angular front end: it draws screens, it owns no tables.
    expect(within(dialog).getByText(/Project: screens only/)).toBeInTheDocument();
    expect(within(dialog).queryByLabelText(/Which tables and columns/)).not.toBeInTheDocument();
    const contract = within(dialog).getByLabelText(/Which backend endpoints does this call/);
    await user.type(contract, 'POST /subscriptions/cancel');
    await user.click(within(dialog).getByRole('button', { name: 'Create' }));

    const sent = vi.mocked(createSpec).mock.calls[0]?.[0];
    expect(sent?.context).toBe(
      'Answers from the person:\n- Which backend endpoints does this call? What are the request and the response, and the errors?\n  POST /subscriptions/cancel',
    );
  });

  it('offers the workspace’s integrations for testing and review, chosen when they are on, and sends the choice', async () => {
    backend();
    vi.mocked(listSpecs).mockResolvedValue([]);
    const stdio = { kind: 'stdio' as const, executable: 'npx', args: [], env: [] };
    const connection = (id: string, name: string, enabled: boolean) => ({
      id,
      workspaceId: 'ws1',
      name,
      transport: stdio,
      enabled,
      required: false,
      secrets: {},
      createdAt: 1,
    });
    vi.mocked(listMcpConnections).mockResolvedValue({
      connections: [
        {
          ...connection('c1', 'chrome-devtools', true),
          discovery: {
            discoveredAt: 1,
            runtimeId: 'claude',
            status: 'connected',
            tools: ['a', 'b'],
          },
        },
        connection('c2', 'jira', false),
      ],
      grants: [],
    });
    const draft = view('draft', { started: false });
    vi.mocked(createSpec).mockResolvedValue(draft);
    vi.mocked(getSpec).mockResolvedValue(draft);
    const user = userEvent.setup();
    renderWithProviders(<SpecsPage />);
    await user.click(first(await screen.findAllByRole('button', { name: 'New specification' })));
    const dialog = await screen.findByRole('dialog', { name: 'New specification' });

    const devtools = await within(dialog).findByRole('checkbox', { name: 'chrome-devtools' });
    expect(devtools).toBeChecked();
    expect(within(dialog).getByText('2 tools')).toBeInTheDocument();
    // One that is switched off cannot be chosen: nothing would be given to anyone.
    expect(within(dialog).getByRole('checkbox', { name: 'jira' })).toBeDisabled();
    expect(within(dialog).getByText(/Choosing one grants nothing/)).toBeInTheDocument();

    await user.type(within(dialog).getByLabelText('Title'), 'Cancel plan');
    await user.type(within(dialog).getByLabelText(/What do you want built/), 'Cancel a plan');
    await user.click(within(dialog).getByRole('button', { name: 'Create' }));
    expect(vi.mocked(createSpec).mock.calls[0]?.[0].integrations).toEqual(['c1']);
  });

  it('shows the core’s reason when the request is refused', async () => {
    backend();
    vi.mocked(listSpecs).mockResolvedValue([]);
    vi.mocked(createSpec).mockRejectedValue(
      new CommandError('create_spec', {
        code: 'spec_invalid',
        params: { reason: 'agent_invalid' },
        detail: null,
      }),
    );
    const user = userEvent.setup();
    renderWithProviders(<SpecsPage />);
    await user.click(first(await screen.findAllByRole('button', { name: 'New specification' })));
    const dialog = await screen.findByRole('dialog', { name: 'New specification' });
    await user.type(within(dialog).getByLabelText('Title'), 'T');
    await user.type(within(dialog).getByLabelText(/What do you want built/), 'R');
    await user.click(within(dialog).getByRole('button', { name: 'Create' }));

    expect(await within(dialog).findByRole('alert')).toHaveTextContent(
      'cannot be created like this (agent_invalid)',
    );
    expect(within(dialog).getByRole('button', { name: 'Create' })).toBeEnabled();
  });
});

describe('reviewing a document', () => {
  it('shows the document, its checks and the decisions, in that order of importance', async () => {
    await open(proposalWritten());

    expect(screen.getByText(/Night users find the bright interface tiring/)).toBeInTheDocument();
    expect(screen.getByText('proposal.md')).toBeInTheDocument();
    expect(screen.getAllByText(/Revision 1 · written by the agent/).length).toBeGreaterThan(0);
    expect(screen.getByText('Passed')).toBeInTheDocument();
    for (const name of ['Approve and continue', 'Request changes', 'Write again']) {
      expect(screen.getByRole('button', { name })).toBeEnabled();
    }
    const steps = screen.getByRole('navigation', { name: 'Stages' });
    expect(within(steps).getByText('Awaiting review')).toBeInTheDocument();
    expect(within(steps).getAllByText('Waiting for earlier documents')).toHaveLength(3);
    expect(screen.getByText(/Approving lets Requirements be written/)).toBeInTheDocument();
  });

  it('puts the open questions of a document up front, and sends the answers as a change request', async () => {
    const user = await open(
      view('awaiting_review', {
        artifacts: [
          artifact('proposal', {
            review: 'awaiting_review',
            files: [
              file('proposal.md', '# Proposal\n\n## Open Questions\n- How long is the timeout?\n'),
            ],
          }),
          artifact('specs'),
          artifact('design'),
          artifact('tasks'),
        ],
      }),
    );
    vi.mocked(decideArtifact).mockResolvedValue({ view: proposalWritten(), launchError: null });

    const callout = screen.getByRole('region', { name: 'Answer these questions' });
    expect(within(callout).getByText('How long is the timeout?')).toBeInTheDocument();
    // Approving is still possible: the warning is not a gate, and it does not go away by itself.
    expect(screen.getByRole('button', { name: 'Approve and continue' })).toBeEnabled();

    await user.click(within(callout).getByRole('button', { name: 'Answer these questions' }));
    const dialog = await screen.findByRole('dialog', { name: 'Answer the open questions' });
    const send = within(dialog).getByRole('button', { name: 'Send answers to the agent' });
    expect(send).toBeDisabled();
    await user.type(within(dialog).getByRole('textbox', { name: /timeout/ }), 'Ten seconds');
    await user.click(send);

    expect(decideArtifact).toHaveBeenCalledWith(
      'spec-1',
      'proposal',
      1,
      'revise',
      expect.stringContaining('Question: How long is the timeout?\nAnswer: Ten seconds'),
    );
  });

  it('shows no warning for a document without open questions', async () => {
    await open(proposalWritten());

    expect(
      screen.queryByRole('region', { name: 'Answer these questions' }),
    ).not.toBeInTheDocument();
  });

  it('approves the revision it looked at, once, however many times it is clicked', async () => {
    const user = await open(proposalWritten());
    let finish: (value: { view: SpecViewDto; launchError: null }) => void = () => undefined;
    vi.mocked(decideArtifact).mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );

    const approve = screen.getByRole('button', { name: 'Approve and continue' });
    await user.click(approve);
    await user.click(approve);

    expect(decideArtifact).toHaveBeenCalledTimes(1);
    expect(decideArtifact).toHaveBeenCalledWith('spec-1', 'proposal', 1, 'approve', null);
    expect(approve).toBeDisabled();
    finish({
      view: view('generating', {
        artifacts: [
          artifact('proposal', { approved: true }),
          artifact('specs', {
            generation: { state: 'running', runId: 'e', startedAt: Date.now(), mode: 'initial' },
          }),
          artifact('design'),
          artifact('tasks'),
        ],
      }),
      launchError: null,
    });
    // The next document is the one in front of the person now, with the run shown as it is.
    expect(await screen.findByText(/Architect is writing the Requirements/)).toBeInTheDocument();
  });

  it('asks for changes only with words, and sends them', async () => {
    const user = await open(proposalWritten());
    vi.mocked(decideArtifact).mockResolvedValue({ view: proposalWritten(), launchError: null });

    await user.click(screen.getByRole('button', { name: 'Request changes' }));
    const dialog = await screen.findByRole('dialog', { name: 'Request changes' });
    expect(within(dialog).getByText('proposal.md')).toBeInTheDocument();
    const send = within(dialog).getByRole('button', { name: 'Send to the agent' });
    expect(send).toBeDisabled();

    await user.type(within(dialog).getByRole('textbox'), 'Mention night shift workers');
    await user.click(send);

    expect(decideArtifact).toHaveBeenCalledWith(
      'spec-1',
      'proposal',
      1,
      'revise',
      'Mention night shift workers',
    );
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Request changes' })).not.toBeInTheDocument();
    });
  });

  it('has the document written again without needing words', async () => {
    const user = await open(proposalWritten());
    vi.mocked(decideArtifact).mockResolvedValue({ view: proposalWritten(), launchError: null });

    await user.click(screen.getByRole('button', { name: 'Write again' }));
    const dialog = await screen.findByRole('dialog', { name: 'Write this document again' });
    await user.click(within(dialog).getByRole('button', { name: 'Write again' }));

    expect(decideArtifact).toHaveBeenCalledWith('spec-1', 'proposal', 1, 'redo', null);
  });

  it('will not approve a document the checks found errors in, and says why', async () => {
    await open(
      view('awaiting_review', {
        artifacts: [
          artifact('proposal', { issues: [issue('why_too_short', 'error', { min: '50' })] }),
          artifact('specs'),
          artifact('design'),
          artifact('tasks'),
        ],
      }),
    );

    expect(screen.getByRole('button', { name: 'Approve and continue' })).toBeDisabled();
    expect(
      screen.getAllByText(/The checks found errors: ask for changes first/).length,
    ).toBeGreaterThan(0);
    expect(screen.getByText('“Why” is too short: at least 50 characters.')).toBeInTheDocument();
    expect(screen.getByText('1 error(s)')).toBeInTheDocument();
  });

  it('keeps the person on the refusal when the core says the document changed', async () => {
    const user = await open(proposalWritten());
    vi.mocked(decideArtifact).mockRejectedValue(
      new CommandError('decide_spec_artifact', {
        code: 'spec_revision_outdated',
        params: { current: '2' },
        detail: null,
      }),
    );

    await user.click(screen.getByRole('button', { name: 'Approve and continue' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Read revision 2 and decide again');
    expect(screen.getByRole('button', { name: 'Approve and continue' })).toBeEnabled();
  });

  it('reads the spec files as requirements and scenarios, and also as the Markdown they are', async () => {
    const user = await open(
      view(
        'awaiting_review',
        {
          artifacts: [
            artifact('proposal', { approved: true }),
            artifact('specs', { review: 'awaiting_review' }),
            artifact('design'),
            artifact('tasks'),
          ],
        },
        {
          outline: [
            {
              path: 'specs/dark-mode/spec.md',
              purpose: 'Lets people switch the theme.',
              requirements: [
                {
                  operation: 'ADDED',
                  name: 'User can switch the theme',
                  line: 3,
                  text: 'The system SHALL let the user pick light or dark.',
                  scenarios: [
                    {
                      name: 'Switch to dark',
                      line: 6,
                      steps: [
                        { keyword: 'WHEN', text: 'the user picks dark' },
                        { keyword: 'THEN', text: 'the interface uses the dark theme' },
                      ],
                    },
                  ],
                },
              ],
            },
          ],
        },
      ),
    );

    expect(screen.getByRole('heading', { name: 'User can switch the theme' })).toBeInTheDocument();
    expect(screen.getByText('Switch to dark')).toBeInTheDocument();
    expect(screen.getByText('WHEN')).toBeInTheDocument();
    expect(screen.getByText('the interface uses the dark theme')).toBeInTheDocument();
    expect(screen.getByText('Lets people switch the theme.')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Markdown' }));
    expect(screen.getByText(/### Requirement: User can switch the theme/)).toBeInTheDocument();
  });

  it('shows each spec file of a document on its own tab', async () => {
    const user = await open(
      view('awaiting_review', {
        artifacts: [
          artifact('proposal', { approved: true }),
          artifact('specs', {
            review: 'awaiting_review',
            files: [
              file('specs/auth/spec.md', 'AUTH FILE'),
              file('specs/export/spec.md', 'EXPORT FILE'),
            ],
          }),
          artifact('design'),
          artifact('tasks'),
        ],
      }),
    );

    expect(screen.getByText('AUTH FILE')).toBeInTheDocument();
    await user.click(screen.getByRole('tab', { name: 'export' }));
    expect(screen.getByText('EXPORT FILE')).toBeInTheDocument();
  });

  it('lets an older revision be read, marked as older', async () => {
    const user = await open(
      view('awaiting_review', {
        artifacts: [
          artifact('proposal', { revisions: 2 }),
          artifact('specs'),
          artifact('design'),
          artifact('tasks'),
        ],
      }),
    );

    await user.selectOptions(screen.getByLabelText('Revision'), '1');

    expect(screen.getByText('Older revision')).toBeInTheDocument();
  });
});

describe('what the agents are doing', () => {
  it('shows a run as it is and lets it be cancelled', async () => {
    const user = await open(
      view('generating', {
        artifacts: [
          artifact('proposal', {
            generation: {
              state: 'running',
              runId: 'exec-1',
              startedAt: Date.now(),
              mode: 'initial',
            },
            files: [],
            revisions: 0,
          }),
          artifact('specs'),
          artifact('design'),
          artifact('tasks'),
        ],
      }),
    );
    vi.mocked(cancelRun).mockResolvedValue();

    expect(screen.getByText('Architect is writing the Proposal')).toBeInTheDocument();
    expect(screen.getByText(/First draft/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Approve and continue' })).not.toBeInTheDocument();

    await user.click(first(screen.getAllByRole('button', { name: 'Cancel run' })));
    expect(cancelRun).toHaveBeenCalledWith('spec-1');
  });

  it('says why a run failed, keeps the earlier document and offers another try', async () => {
    const user = await open(
      view('failed', {
        artifacts: [
          artifact('proposal', {
            review: 'changes_requested',
            generation: {
              state: 'failed',
              message: 'output_no_document',
              at: Date.now(),
              interrupted: false,
            },
          }),
          artifact('specs'),
          artifact('design'),
          artifact('tasks'),
        ],
      }),
    );
    vi.mocked(generateArtifact).mockResolvedValue({ view: proposalWritten(), launchError: null });

    expect(screen.getByRole('alert')).toHaveTextContent('The agent’s turn failed');
    expect(screen.getByRole('alert')).toHaveTextContent('no document in the agreed format');
    expect(screen.getByRole('alert')).toHaveTextContent('What was written before is untouched');
    expect(screen.getByText(/Night users find the bright interface/)).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Write this document' }));
    expect(generateArtifact).toHaveBeenCalledWith('spec-1', 'proposal');
  });

  it('says a run was interrupted by the app closing', async () => {
    await open(
      view('failed', {
        artifacts: [
          artifact('proposal', {
            generation: {
              state: 'failed',
              message: 'interrupted',
              at: Date.now(),
              interrupted: true,
            },
            files: [],
            revisions: 0,
          }),
          artifact('specs'),
          artifact('design'),
          artifact('tasks'),
        ],
      }),
    );

    expect(screen.getByRole('alert')).toHaveTextContent('interrupted when the app closed');
  });

  it('puts an agent’s question in front of the person and goes on from the answer', async () => {
    const current = view('waiting_for_input', {
      artifacts: [
        artifact('proposal', {
          generation: { state: 'waiting_for_input', questionId: 'q1' },
          files: [],
          revisions: 0,
        }),
        artifact('specs'),
        artifact('design'),
        artifact('tasks'),
      ],
      questions: [
        {
          id: 'q1',
          source: 'artifact',
          artifactId: 'proposal',
          findingId: null,
          question: 'Which users get the dark mode?',
          context: 'The request does not say.',
          options: [
            { id: 'Everyone', label: 'Everyone' },
            { id: 'Admins', label: 'Admins only' },
          ],
          status: 'pending',
          choice: null,
          answer: null,
          runId: 'exec-1',
          askedAt: Date.now(),
          answeredAt: null,
        },
      ],
    });
    const user = await open(current);
    vi.mocked(answerQuestion).mockImplementation(() =>
      Promise.resolve({ view: current, launchError: null }),
    );

    expect(screen.getByText('Which users get the dark mode?')).toBeInTheDocument();
    expect(screen.getByText('The request does not say.')).toBeInTheDocument();
    const send = screen.getByRole('button', { name: 'Send answer and go on' });
    expect(send).toBeDisabled();

    await user.type(screen.getByLabelText('Your answer'), 'Only admins for now');
    await user.click(send);
    expect(answerQuestion).toHaveBeenCalledWith('spec-1', 'q1', null, 'Only admins for now');

    await user.click(screen.getByRole('button', { name: 'Admins only' }));
    expect(answerQuestion).toHaveBeenLastCalledWith(
      'spec-1',
      'q1',
      'Admins',
      'Only admins for now',
    );
  });

  it('says when an agent could not be started after the decision was saved', async () => {
    const user = await open(proposalWritten());
    vi.mocked(decideArtifact).mockResolvedValue({
      view: proposalWritten(),
      launchError: { code: 'agent_busy', params: {}, detail: null },
    });

    await user.click(screen.getByRole('button', { name: 'Approve and continue' }));

    expect(await screen.findByText('Saved, but the agent could not start')).toBeInTheDocument();
  });

  it('will not offer to write while the writing agent is gone, and says why', async () => {
    await open(
      view(
        'awaiting_generation',
        {
          artifacts: [
            artifact('proposal'),
            artifact('specs'),
            artifact('design'),
            artifact('tasks'),
          ],
        },
        { authorAvailable: false },
      ),
    );

    expect(screen.getByRole('button', { name: 'Write this document' })).toBeDisabled();
    expect(screen.getByText(/writing agent is no longer in this workspace/)).toBeInTheDocument();
  });
});

describe('documents that changed under their readers', () => {
  it('marks a document as outdated and offers to keep it or update it', async () => {
    const user = await open(
      view('blocked', {
        artifacts: [
          artifact('proposal', { approved: true }),
          artifact('specs', { approved: true, revisions: 2 }),
          artifact('design', { approved: true, outdated: true }),
          artifact('tasks'),
        ],
      }),
    );
    await user.click(
      within(screen.getByRole('navigation', { name: 'Stages' })).getByRole('button', {
        name: /Design/,
      }),
    );
    vi.mocked(keepArtifact).mockResolvedValue(proposalWritten());
    vi.mocked(generateArtifact).mockResolvedValue({ view: proposalWritten(), launchError: null });

    expect(screen.getAllByText('Outdated').length).toBeGreaterThan(0);
    expect(screen.getByText(/Something this was written from changed/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Approve and continue' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Write this document' })).toBeEnabled();
    await user.click(screen.getByRole('button', { name: 'Keep as it is' }));
    expect(keepArtifact).toHaveBeenCalledWith('spec-1', 'design', 1);
  });

  it('follows the spec as it changes and never goes back to an older version', async () => {
    await open(proposalWritten());
    expect(screen.getByRole('button', { name: 'Approve and continue' })).toBeInTheDocument();

    const newer = view('awaiting_review', {
      version: 5,
      artifacts: [
        artifact('proposal', { approved: true }),
        artifact('specs', { review: 'awaiting_review' }),
        artifact('design'),
        artifact('tasks'),
      ],
    });
    vi.mocked(getSpec).mockResolvedValue(newer);
    emit({ specId: 'spec-1', version: 5 });
    await waitFor(() => {
      expect(screen.getByText(/Approving lets Design be written/)).toBeInTheDocument();
    });

    // A read that was already on its way when something newer arrived changes nothing.
    vi.mocked(getSpec).mockResolvedValue(proposalWritten());
    emit({ specId: 'spec-1', version: 6 });
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(screen.getByText(/Approving lets Design be written/)).toBeInTheDocument();
    // And another spec’s change is not this one’s business.
    vi.mocked(getSpec).mockClear();
    emit({ specId: 'other', version: 9 });
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(getSpec).not.toHaveBeenCalled();
  });

  it('comes back to where the spec stood when it is opened again', async () => {
    const user = await open(proposalWritten());
    await user.click(screen.getByRole('button', { name: 'All specs' }));
    expect(await screen.findByRole('button', { name: 'Dark mode' })).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Dark mode' }));

    expect(await screen.findByText(/Night users find the bright interface/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Approve and continue' })).toBeEnabled();
  });
});

function finalView(
  patch: Partial<SpecViewDto> = {},
  specPatch: Parameters<typeof specDto>[0] = {},
) {
  return view(
    'awaiting_validation',
    { artifacts: allApproved(), ...specPatch },
    {
      readiness: {
        ready: false,
        blockers: [
          { code: 'review_not_run', artifactId: null, count: null },
          { code: 'final_approval_missing', artifactId: null, count: null },
        ],
      },
      consistencyCurrent: true,
      ...patch,
    },
  );
}

describe('the final review', () => {
  it('lands on the review when every document is approved, and lists what is missing', async () => {
    await open(finalView());

    expect(screen.getByText('What is left before this spec is ready')).toBeInTheDocument();
    expect(screen.getByText('Not ready')).toBeInTheDocument();
    expect(screen.getByText('The review has not run')).toBeInTheDocument();
    expect(screen.getByText('Your final approval is missing')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Approve the specification' })).toBeDisabled();
    await openPanel('Build from this spec');
    expect(screen.getByRole('button', { name: 'Start a workflow from this spec' })).toBeDisabled();
  });

  it('runs the review on the person’s say-so', async () => {
    const user = await open(finalView());
    vi.mocked(runReview).mockResolvedValue({ view: finalView(), launchError: null });

    await openPanel('Review by the reviewing agent');
    await user.click(screen.getByRole('button', { name: 'Run the review' }));

    expect(runReview).toHaveBeenCalledWith('spec-1');
  });

  it('shows the findings as opinions and takes the person’s decision on each', async () => {
    const finding = {
      id: 'f1',
      category: 'missing_scenario',
      severity: 'high' as const,
      title: 'No error scenario',
      description: 'Saving the choice can fail and nothing says what happens.',
      artifactId: 'specs',
      reference: 'Requirement: User can switch the theme',
      evidence: 'The system SHALL let the user pick light or dark.',
      rationale: 'Inferred from the persistence in the proposal.',
      question: 'Should the choice persist?',
      suggestion: 'Add a failure scenario.',
      artifactRevision: 1,
      status: 'open' as const,
      resolutionNote: null,
      reviewId: 'r1',
      createdAt: Date.now(),
      resolvedAt: null,
    };
    const current = finalView(
      { reviewCurrent: true },
      {
        findings: [finding],
        aiReview: {
          id: 'r1',
          agentId: 'a1',
          revisions: {},
          run: { state: 'completed', at: Date.now(), summary: 'One gap found.' },
          startedAt: Date.now(),
        },
      },
    );
    const user = await open(current);
    vi.mocked(resolveFinding).mockResolvedValue(current);
    vi.mocked(answerFindings).mockResolvedValue({ view: current, launchError: null });

    expect(screen.getByText('One gap found.')).toBeInTheDocument();
    // Once in the list and once as the finding being read.
    expect(screen.getAllByText('No error scenario')).toHaveLength(2);
    expect(screen.getByText(/1 open · 1 ask you something/)).toBeInTheDocument();
    expect(screen.getAllByText('Missing scenario').length).toBeGreaterThan(0);
    expect(
      screen.getByText(/The reviewing agent’s opinion, not a failed check/),
    ).toBeInTheDocument();
    expect(
      screen.getByText('The system SHALL let the user pick light or dark.'),
    ).toBeInTheDocument();

    // Accepting a risk needs the person's reason.
    await user.click(screen.getByRole('button', { name: 'Accept the risk…' }));
    const confirm = screen.getByRole('button', { name: 'Accept the risk' });
    expect(confirm).toBeDisabled();
    await user.type(screen.getByLabelText(/Why is it acceptable/), 'Ops handles retries');
    await user.click(confirm);
    expect(resolveFinding).toHaveBeenCalledWith(
      'spec-1',
      'f1',
      'accepted_risk',
      'Ops handles retries',
    );

    // Answering is writing: nothing is sent until the person has looked at all of them.
    await user.type(screen.getByLabelText(/Should the choice persist/), 'Yes, across restarts');
    expect(answerFindings).not.toHaveBeenCalled();
    expect(
      screen.getByText('Saved. All your answers are sent together at the end.'),
    ).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Review and send (1)' }));
    const send = await screen.findByRole('dialog', { name: 'Send your answers' });
    await user.click(within(send).getByRole('button', { name: 'Send 1 answer(s) and revise' }));
    expect(answerFindings).toHaveBeenCalledWith('spec-1', [
      { findingId: 'f1', answer: 'Yes, across restarts' },
    ]);
  });

  it('lists the findings beside the one being read, the ones that ask you first, and shows the one picked', async () => {
    const base = {
      category: 'ambiguity',
      artifactId: 'specs',
      reference: '',
      evidence: '',
      rationale: '',
      suggestion: '',
      artifactRevision: 1,
      status: 'open' as const,
      resolutionNote: null,
      reviewId: 'r1',
      createdAt: Date.now(),
      resolvedAt: null,
    };
    const findings = [
      {
        ...base,
        id: 'f1',
        severity: 'low' as const,
        title: 'Wording is loose',
        description: 'd1',
        question: '',
      },
      {
        ...base,
        id: 'f2',
        severity: 'medium' as const,
        title: 'Which timeout?',
        description: 'd2',
        question: 'How many seconds?',
      },
    ];
    const withBoth = finalView(
      { reviewCurrent: true },
      {
        findings,
        aiReview: {
          id: 'r1',
          agentId: 'a1',
          revisions: {},
          run: { state: 'completed', at: Date.now(), summary: 'Two gaps.' },
          startedAt: Date.now(),
        },
      },
    );
    const user = await open(withBoth);

    const list = screen.getByRole('list', { name: 'Review findings' });
    const titles = within(list)
      .getAllByRole('button')
      .map((b) => b.textContent);
    expect(titles[0]).toContain('Which timeout?');
    expect(titles[0]).toContain('asks you');
    expect(screen.getByLabelText(/How many seconds/)).toBeVisible();

    const [, second] = within(list).getAllByRole('button');
    if (second) await user.click(second);
    expect(screen.getByLabelText(/How many seconds/)).not.toBeVisible();
  });

  it('keeps every answer while the person moves between findings, and sends them all together', async () => {
    const base = {
      category: 'ambiguity',
      artifactId: 'specs',
      reference: '',
      evidence: '',
      rationale: '',
      suggestion: '',
      artifactRevision: 1,
      status: 'open' as const,
      resolutionNote: null,
      reviewId: 'r1',
      createdAt: Date.now(),
      resolvedAt: null,
      severity: 'medium' as const,
      description: 'd',
    };
    const findings = [
      { ...base, id: 'f1', title: 'One', question: 'First question?' },
      { ...base, id: 'f2', title: 'Two', question: 'Second question?' },
    ];
    const current = finalView(
      { reviewCurrent: true },
      {
        findings,
        aiReview: {
          id: 'r1',
          agentId: 'a1',
          revisions: {},
          run: { state: 'completed', at: Date.now(), summary: 'Gaps.' },
          startedAt: Date.now(),
        },
      },
    );
    const user = await open(current);
    vi.mocked(answerFindings).mockResolvedValue({ view: current, launchError: null });
    const list = screen.getByRole('list', { name: 'Review findings' });
    const [first, second] = within(list).getAllByRole('button');

    await user.type(screen.getByLabelText(/First question/), 'Answer one');
    if (second) await user.click(second);
    await user.type(screen.getByLabelText(/Second question/), 'Answer two');
    if (first) await user.click(first);
    // Going back finds what was written, and nothing has been sent.
    expect(screen.getByLabelText(/First question/)).toHaveValue('Answer one');
    expect(answerFindings).not.toHaveBeenCalled();
    expect(screen.getAllByText('2 of 2 answered').length).toBeGreaterThan(0);

    await user.click(screen.getByRole('button', { name: 'Review and send (2)' }));
    const dialog = await screen.findByRole('dialog', { name: 'Send your answers' });
    await user.click(within(dialog).getByRole('button', { name: 'Send 2 answer(s) and revise' }));

    expect(answerFindings).toHaveBeenCalledTimes(1);
    expect(answerFindings).toHaveBeenCalledWith('spec-1', [
      { findingId: 'f1', answer: 'Answer one' },
      { findingId: 'f2', answer: 'Answer two' },
    ]);
  });

  it('says what the spec has used, as the runtimes reported it, and what it could not tell', async () => {
    const current = finalView({
      usage: {
        runs: [],
        inputTokens: 120_000,
        outputTokens: 30_000,
        cachedInputTokens: 80_000,
        cost: 1.5,
        currency: 'USD',
        runsTotal: 5,
        runsMeasured: 4,
      },
    });
    await open(current);

    expect(screen.getByText('120K in · 30K out')).toBeInTheDocument();
    expect(screen.getByText('80K from cache')).toBeInTheDocument();
    expect(screen.getByText('$1.50')).toBeInTheDocument();
    expect(screen.getByText('4 of 5 runs reported numbers.')).toBeInTheDocument();
  });

  it('stops at the token limit and goes on only when the person says so', async () => {
    const reached = finalView({
      usage: {
        runs: [],
        inputTokens: 90_000,
        outputTokens: 20_000,
        cachedInputTokens: null,
        cost: null,
        currency: null,
        runsTotal: 3,
        runsMeasured: 3,
      },
      budget: { limit: 100_000, spent: 110_000, steps: 0, reached: true, near: true },
    });
    const user = await open(reached);
    vi.mocked(raiseBudget).mockResolvedValue(finalView());

    expect(screen.getByRole('alert')).toHaveTextContent('reached its token limit');
    await user.click(screen.getByRole('button', { name: 'Go on (+100K)' }));

    expect(raiseBudget).toHaveBeenCalledWith('spec-1');
  });

  it('shows no usage bar for a spec that reported nothing and has no limit', async () => {
    await open(finalView());

    expect(screen.queryByText('Usage')).not.toBeInTheDocument();
  });

  it('lets the review be skipped only in writing', async () => {
    const user = await open(finalView());
    vi.mocked(approveSpec).mockResolvedValue(finalView());

    await openPanel('Review by the reviewing agent');
    await user.click(screen.getByRole('button', { name: 'Go without the review…' }));
    const skip = screen.getByRole('button', { name: 'Skip the review' });
    expect(skip).toBeDisabled();
    await user.type(screen.getByLabelText(/Why skip it/), 'Tiny change');
    expect(skip).toBeEnabled();
  });

  it('gives the last word only when everything else holds, and says what it is worth', async () => {
    const user = await open(
      finalView({
        readiness: {
          ready: false,
          blockers: [{ code: 'final_approval_missing', artifactId: null, count: null }],
        },
        reviewCurrent: true,
      }),
    );
    vi.mocked(approveSpec).mockResolvedValue(finalView());

    const approve = screen.getByRole('button', { name: 'Approve the specification' });
    expect(approve).toBeEnabled();
    await user.click(approve);
    expect(approveSpec).toHaveBeenCalledWith('spec-1');
  });

  it('goes back to the document that is in the way', async () => {
    const user = await open(
      finalView({
        readiness: {
          ready: false,
          blockers: [{ code: 'artifact_outdated', artifactId: 'design', count: null }],
        },
      }),
    );

    await user.click(screen.getByRole('button', { name: 'Open' }));

    expect(await screen.findByText(/Text of the design/)).toBeInTheDocument();
  });

  it('runs the checks again when asked', async () => {
    const user = await open(finalView());
    vi.mocked(validateSpec).mockResolvedValue(finalView());

    await user.click(first(screen.getAllByRole('button', { name: 'Run checks again' })));

    expect(validateSpec).toHaveBeenCalledWith('spec-1');
  });
});

describe('publishing and building', () => {
  const ready = () =>
    view(
      'ready_for_implementation',
      {
        artifacts: allApproved(),
        finalApproval: { revisions: {}, acknowledgedWarnings: [], at: Date.now() },
      },
      {
        readiness: { ready: true, blockers: [] },
        reviewCurrent: true,
        consistencyCurrent: true,
        finalApprovalCurrent: true,
      },
    );

  it('publishes under the name in the field, and says nothing is overwritten', async () => {
    const user = await open(ready());
    vi.mocked(publishSpec).mockResolvedValue(ready());

    await openPanel('Publish to the project');
    expect(screen.getByText(/Existing files are never overwritten/)).toBeInTheDocument();
    await user.clear(screen.getByLabelText(/Change folder name/));
    await user.type(screen.getByLabelText(/Change folder name/), 'dark-mode-v2');
    await user.click(screen.getByRole('button', { name: 'Publish' }));

    expect(publishSpec).toHaveBeenCalledWith('spec-1', 'dark-mode-v2');
  });

  it('says the folder is taken, that nothing was written, and keeps the name to change', async () => {
    const user = await open(ready());
    vi.mocked(publishSpec).mockRejectedValue(
      new CommandError('publish_spec', {
        code: 'spec_change_exists',
        params: { change: 'dark-mode' },
        detail: null,
      }),
    );

    await openPanel('Publish to the project');
    await user.click(screen.getByRole('button', { name: 'Publish' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'The folder “dark-mode” already exists in the project. Nothing was written; choose another name.',
    );
    expect(screen.getByLabelText(/Change folder name/)).toHaveValue('dark-mode');
  });

  it('says a publication was left half way and offers to continue it', async () => {
    await open(
      view(
        'blocked',
        {
          artifacts: allApproved(),
          publication: {
            state: 'failed',
            message: 'disk full',
            at: Date.now(),
            changeId: 'dark-mode',
            written: ['proposal.md'],
          },
        },
        {
          readiness: {
            ready: false,
            blockers: [{ code: 'publication_failed', artifactId: null, count: null }],
          },
          finalApprovalCurrent: true,
          reviewCurrent: true,
          consistencyCurrent: true,
        },
      ),
    );

    expect(screen.getByText(/Stopped after 1 file\(s\)/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Continue publishing' })).toBeEnabled();
  });

  it('hands a ready spec to a workflow', async () => {
    const user = await open(ready());

    await openPanel('Build from this spec');
    await user.click(screen.getByRole('button', { name: 'Start a workflow from this spec' }));

    expect(navigate).toHaveBeenCalledWith('workflow', {
      type: 'workflow-from-spec',
      specId: 'spec-1',
    });
  });

  it('reads an archived spec and changes nothing in it', async () => {
    await open(
      view(
        'archived',
        { artifacts: allApproved(), archived: { at: Date.now(), path: null, method: 'atlas' } },
        { readiness: { ready: true, blockers: [] } },
      ),
    );

    await openPanel('Build from this spec');
    expect(screen.getByRole('button', { name: 'Start a workflow from this spec' })).toBeDisabled();
    await openPanel('Publish to the project');
    expect(screen.getByRole('button', { name: 'Publish' })).toBeDisabled();
  });
});

describe('the optional OpenSpec command', () => {
  const published = (extra: Partial<SpecViewDto> = {}) =>
    view(
      'published',
      {
        artifacts: allApproved(),
        publication: {
          state: 'published',
          changeId: 'dark-mode',
          root: 'openspec/changes/dark-mode',
          files: ['proposal.md'],
          revisions: {},
          at: Date.now(),
        },
      },
      {
        readiness: { ready: true, blockers: [] },
        reviewCurrent: true,
        consistencyCurrent: true,
        finalApprovalCurrent: true,
        publicationCurrent: true,
        ...extra,
      },
    );

  it('says it is not installed and that nothing else needs it, and offers nothing it cannot do', async () => {
    vi.mocked(getCliStatus).mockResolvedValue({
      available: false,
      version: null,
      compatible: false,
      reason: 'not_installed',
    });
    await open(published());

    expect(await screen.findByText(/The openspec command is not installed/)).toBeInTheDocument();
    await openPanel('OpenSpec command (optional)');
    expect(screen.getByRole('button', { name: 'Check with OpenSpec' })).toBeDisabled();
    await openPanel('OpenSpec command (optional)');
    expect(screen.getByRole('button', { name: 'Archive with OpenSpec…' })).toBeDisabled();
    // Everything Atlas itself does is still on offer.
    await openPanel('Publish to the project');
    expect(screen.getByRole('button', { name: 'Publish' })).toBeInTheDocument();
  });

  it('gives a second opinion on the published change, apart from Atlas’s own checks', async () => {
    vi.mocked(getCliStatus).mockResolvedValue({
      available: true,
      version: '1.3.1',
      compatible: true,
      reason: null,
    });
    const checked = published();
    checked.spec.cliCheck = {
      revisions: {},
      cliVersion: '1.3.1',
      valid: false,
      issues: [
        { level: 'ERROR', path: 'a/spec.md', message: 'ADDED "Weak" must contain SHALL or MUST' },
      ],
      at: Date.now(),
    };
    const user = await open(published());
    vi.mocked(checkWithCli).mockResolvedValue(checked);

    await openPanel('OpenSpec command (optional)');
    await user.click(await screen.findByRole('button', { name: 'Check with OpenSpec' }));

    expect(checkWithCli).toHaveBeenCalledWith('spec-1');
    expect(
      await screen.findByText(/finds problems in the published change \(v1.3.1\)/),
    ).toBeInTheDocument();
    expect(screen.getByText('ADDED "Weak" must contain SHALL or MUST')).toBeInTheDocument();
    expect(screen.getByText(/does not change whether the spec is ready/)).toBeInTheDocument();
  });

  it('says what archiving will do and does nothing until it is confirmed', async () => {
    vi.mocked(getCliStatus).mockResolvedValue({
      available: true,
      version: '1.3.1',
      compatible: true,
      reason: null,
    });
    const user = await open(published());
    vi.mocked(archiveWithCli).mockResolvedValue(published());

    await openPanel('OpenSpec command (optional)');
    await user.click(await screen.findByRole('button', { name: 'Archive with OpenSpec…' }));
    const dialog = await screen.findByRole('dialog', {
      name: 'Archive with the OpenSpec command?',
    });
    expect(within(dialog).getByText(/merges the change’s requirements/)).toBeInTheDocument();
    expect(within(dialog).getByText(/moves openspec\/changes\/dark-mode/)).toBeInTheDocument();
    expect(archiveWithCli).not.toHaveBeenCalled();

    await user.click(within(dialog).getByRole('button', { name: 'Archive' }));
    expect(archiveWithCli).toHaveBeenCalledWith('spec-1');
  });

  it('shows what the command said when it refuses', async () => {
    vi.mocked(getCliStatus).mockResolvedValue({
      available: true,
      version: '1.3.1',
      compatible: true,
      reason: null,
    });
    const user = await open(published());
    vi.mocked(archiveWithCli).mockRejectedValue(
      new CommandError('archive_spec_with_cli', {
        code: 'spec_cli_failed',
        params: {},
        detail: 'Validation failed: requirement exists',
      }),
    );

    await openPanel('OpenSpec command (optional)');
    await user.click(await screen.findByRole('button', { name: 'Archive with OpenSpec…' }));
    await user.click(
      within(await screen.findByRole('dialog')).getByRole('button', { name: 'Archive' }),
    );

    expect(await screen.findByText('What the command said')).toBeInTheDocument();
    expect(screen.getByText('Validation failed: requirement exists')).toBeInTheDocument();
  });
});

describe('importing', () => {
  it('lists what the project has, with what each lacks, and brings one in', async () => {
    backend();
    vi.mocked(listSpecs).mockResolvedValue([]);
    vi.mocked(listImportable).mockResolvedValue([
      {
        changeId: 'add-export',
        artifacts: ['proposal', 'specs'],
        problems: ['missing:design', 'unrecognized:notes.txt'],
        importedAs: null,
      },
      { changeId: 'old-change', artifacts: ['proposal'], problems: [], importedAs: 'spec-9' },
    ]);
    const imported = view('awaiting_review', { changeId: 'add-export', title: 'Add export' });
    vi.mocked(importSpec).mockResolvedValue(imported);
    vi.mocked(getSpec).mockResolvedValue(imported);
    const user = userEvent.setup();
    renderWithProviders(<SpecsPage />);

    await user.click(first(await screen.findAllByRole('button', { name: 'Import specs' })));
    const dialog = await screen.findByRole('dialog', { name: 'Import specs' });
    expect(await within(dialog).findByText('add-export')).toBeInTheDocument();
    expect(within(dialog).getByText('No Design document')).toBeInTheDocument();
    expect(within(dialog).getByText('File Atlas does not read: notes.txt')).toBeInTheDocument();
    expect(within(dialog).getByText('Already a spec here')).toBeInTheDocument();
    expect(within(dialog).getByText(/only read/)).toBeInTheDocument();

    await user.click(within(dialog).getByRole('button', { name: 'Import' }));

    expect(importSpec).toHaveBeenCalledWith('ws1', 'ws1-main', 'add-export', 'a1', null);
    expect(
      await screen.findByRole('heading', { level: 2, name: 'Add export' }),
    ).toBeInTheDocument();
  });

  it('says there is nothing to import', async () => {
    backend();
    vi.mocked(listSpecs).mockResolvedValue([]);
    vi.mocked(listImportable).mockResolvedValue([]);
    const user = userEvent.setup();
    renderWithProviders(<SpecsPage />);

    await user.click(first(await screen.findAllByRole('button', { name: 'Import specs' })));

    expect(
      await screen.findByText('No OpenSpec change found in this project.'),
    ).toBeInTheDocument();
  });
});
