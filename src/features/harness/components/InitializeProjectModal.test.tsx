import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {
  agent,
  emitHarness,
  harnessSummary,
  mockBackend,
  outcome,
  projectAnalysis,
  renderWithProviders,
  workspace,
} from '@/test/fixtures';
import type { ConflictDto, FindingDto } from '@/lib/tauri/commands';
import {
  analyzeProject,
  initializeProject,
  interruptExecution,
} from '@/features/workspace/services/workspaceService';
import { InitializeProjectModal } from './InitializeProjectModal';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');

const initialized = harnessSummary({
  status: 'initialized',
  projectName: 'transport-erp',
  stack: ['TypeScript', 'Angular 21'],
  version: 1,
  hasAtlasDir: true,
  health: { state: 'healthy', reasons: [] },
});

function open(options = {}) {
  mockBackend({
    workspaces: [workspace('w1', 'ERP', '/dev/transport-erp')],
    agents: [agent('a1', 'Analyst')],
    ...options,
  });
  const onClose = vi.fn();
  const onInitialized = vi.fn();
  renderWithProviders(
    <InitializeProjectModal workspaceId="w1" onClose={onClose} onInitialized={onInitialized} />,
  );
  return { onClose, onInitialized };
}

const modelFinding = (extra: Partial<FindingDto> = {}): FindingDto => ({
  id: 'module:orders',
  category: 'module',
  key: 'orders',
  label: 'orders: Order handling',
  value: 'Order handling',
  confidence: 'medium',
  origin: 'inference',
  reason: 'orders.service.ts lives in application',
  evidence: [{ source: 'src/application/orders.service.ts' }],
  byModel: true,
  ...extra,
});

const conflict: ConflictDto = {
  findingId: 'framework:angular',
  label: 'Angular 21',
  claims: [
    {
      value: 'Angular 21',
      choice: '21',
      origin: 'fact',
      evidence: [{ source: 'package.json', field: 'dependencies["@angular/core"]' }],
    },
    {
      value: 'angular 18',
      choice: '18',
      origin: 'inference',
      evidence: [{ source: 'README.md', field: 'mentions angular 18' }],
    },
  ],
  resolution: null,
};

describe('InitializeProjectModal', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows what was detected, with the architecture only as a possibility', async () => {
    open();

    const detected = await screen.findByRole('region', { name: 'Detected' });
    expect(within(detected).getByText(/Angular 21/)).toBeInTheDocument();
    expect(within(detected).getByText(/GitHub Actions/)).toBeInTheDocument();
    const architecture = screen.getByRole('region', { name: 'Possible architecture' });
    expect(within(architecture).getByText(/Layered/)).toBeInTheDocument();
    expect(within(architecture).getByText('Medium confidence')).toBeInTheDocument();
    const repository = screen.getByRole('region', { name: 'Repository' });
    expect(within(repository).getByText(/Current branch main/)).toBeInTheDocument();
    // Looking is not initializing, and no model was asked.
    expect(initializeProject).not.toHaveBeenCalled();
    expect(analyzeProject).toHaveBeenCalledWith('w1');
  });

  it('says so when the architecture could not be determined', async () => {
    const analysis = projectAnalysis();
    analysis.findings = analysis.findings.filter((f) => f.category !== 'architecture');
    open({ analysis });

    expect(await screen.findByText(/Architecture could not be determined/)).toBeInTheDocument();
  });

  it('warns that the analysis was partial and notes .env files without showing them', async () => {
    const analysis = projectAnalysis({ partial: true });
    analysis.findings.push({
      id: 'environment:env_file',
      category: 'environment',
      key: 'env_file',
      label: '.env file',
      value: 'true',
      confidence: 'high',
      origin: 'fact',
      evidence: [{ source: '.env', field: '.env detected' }],
      byModel: false,
    });
    open({ analysis });

    expect(await screen.findByRole('status')).toHaveTextContent('Analysis partially completed');
    expect(screen.getByText(/\.env file detected \(its contents are never read\)/)).toBeVisible();
  });

  it('reviews, applies corrections, exclusions and confirmations, and initializes only on confirmation', async () => {
    const user = userEvent.setup();
    const { onInitialized } = open();
    vi.mocked(initializeProject).mockResolvedValue(
      outcome(initialized, { written: ['project.yaml'] }),
    );

    await user.click(await screen.findByRole('button', { name: 'Review' }));
    const form = screen.getByRole('form', { name: 'Review Project Context' });
    await user.clear(within(form).getByLabelText('Value of Angular 21'));
    await user.type(within(form).getByLabelText('Value of Angular 21'), '19');
    await user.click(within(form).getByLabelText('Include Vitest'));
    await user.click(within(form).getByLabelText('Confirm Layered'));
    await user.type(within(form).getByLabelText('What does this system do?'), 'ERP for transport');
    await user.type(within(form).getByLabelText('Who uses it?'), 'About 110 internal users');
    await user.type(
      within(form).getByLabelText('Are there architectural constraints we should preserve?'),
      'Keep the public API stable',
    );
    await user.type(
      within(form).getByLabelText(/Decisions to preserve/),
      'The layered architecture is intentional',
    );
    expect(initializeProject).not.toHaveBeenCalled();
    await user.click(within(form).getByRole('button', { name: 'Initialize' }));

    expect(initializeProject).toHaveBeenCalledWith('w1', {
      mode: 'create',
      excluded: ['testing:vitest'],
      corrections: { 'framework:angular': '19' },
      confirmed: ['architecture:layered'],
      purpose: 'ERP for transport',
      users: 'About 110 internal users',
      concepts: '',
      businessRules: '',
      constraints: 'Keep the public API stable',
      decisions: 'The layered architecture is intentional',
    });
    expect(await screen.findByRole('heading', { name: 'Harness initialized' })).toBeVisible();
    expect(screen.getByText('Agents can now use project context.')).toBeVisible();
    expect(onInitialized).toHaveBeenCalledWith(initialized);
  });

  it('shows the evidence behind a statement: confidence, origin and sources', async () => {
    const user = userEvent.setup();
    const analysis = projectAnalysis();
    analysis.findings.push(modelFinding());
    open({ analysis });

    await user.click(await screen.findByRole('button', { name: 'Review' }));
    const form = screen.getByRole('form', { name: 'Review Project Context' });
    // A fact from the repository.
    await user.click(within(form).getByRole('button', { name: 'Show evidence for Angular 21' }));
    const fact = within(form).getByRole('region', { name: 'Evidence for Angular 21' });
    expect(within(fact).getByText('High confidence')).toBeVisible();
    expect(within(fact).getByText('Read from the repository')).toBeVisible();
    expect(within(fact).getByText('package.json')).toBeVisible();
    // An inference from a model: its reason and the files it cites.
    await user.click(
      within(form).getByRole('button', { name: 'Show evidence for orders: Order handling' }),
    );
    const inference = within(form).getByRole('region', {
      name: 'Evidence for orders: Order handling',
    });
    expect(within(inference).getByText('Medium confidence')).toBeVisible();
    expect(within(inference).getByText(/Inferred · proposed by a model/)).toBeVisible();
    expect(within(inference).getByText('orders.service.ts lives in application')).toBeVisible();
    expect(within(inference).getByText('src/application/orders.service.ts')).toBeVisible();
  });

  it('a low-confidence guess starts unticked and cannot be mistaken for a fact', async () => {
    const user = userEvent.setup();
    const analysis = projectAnalysis();
    analysis.findings.push(
      modelFinding({
        id: 'convention:naming',
        category: 'convention',
        key: 'naming',
        label: 'naming: kebab-case',
        confidence: 'low',
      }),
    );
    open({ analysis });
    vi.mocked(initializeProject).mockResolvedValue(outcome(initialized));

    await user.click(await screen.findByRole('button', { name: 'Review' }));
    const guess = screen.getByLabelText('Include naming: kebab-case');
    expect(guess).not.toBeChecked();
    expect(screen.getAllByText('Low confidence').length).toBeGreaterThan(0);
    await user.click(screen.getByRole('button', { name: 'Initialize' }));

    expect(initializeProject).toHaveBeenCalledWith(
      'w1',
      expect.objectContaining({ excluded: ['convention:naming'] }),
    );
  });

  it('shows contradicting evidence on both sides and records the user’s choice as a correction', async () => {
    const user = userEvent.setup();
    open({ analysis: projectAnalysis({ conflicts: [conflict] }) });
    vi.mocked(initializeProject).mockResolvedValue(outcome(initialized));

    const panel = await screen.findByRole('region', { name: 'Conflicting information' });
    expect(within(panel).getByText(/Not decided yet/)).toBeVisible();
    expect(within(panel).getByText(/package\.json/)).toBeVisible();
    expect(within(panel).getByText(/README\.md/)).toBeVisible();
    await user.click(screen.getByRole('button', { name: 'Review' }));
    await user.click(screen.getByRole('button', { name: 'Use Angular 21 for Angular 21' }));
    expect(screen.getByText('Decided: 21')).toBeVisible();
    await user.click(screen.getByRole('button', { name: 'Initialize' }));

    // The fact the user chose is the analysis's own value: no correction needed.
    expect(initializeProject).toHaveBeenCalledWith(
      'w1',
      expect.objectContaining({ corrections: {} }),
    );
  });

  it('choosing the other side of a conflict becomes a correction', async () => {
    const user = userEvent.setup();
    open({ analysis: projectAnalysis({ conflicts: [conflict] }) });
    vi.mocked(initializeProject).mockResolvedValue(outcome(initialized));

    await user.click(await screen.findByRole('button', { name: 'Review' }));
    await user.click(screen.getByRole('button', { name: 'Use angular 18 for Angular 21' }));
    await user.click(screen.getByRole('button', { name: 'Initialize' }));

    expect(initializeProject).toHaveBeenCalledWith(
      'w1',
      expect.objectContaining({ corrections: { 'framework:angular': '18' } }),
    );
  });

  it('starts from the user’s own words and warns about files it will not rewrite', async () => {
    const user = userEvent.setup();
    open({
      analysis: projectAnalysis({
        user: {
          purpose: 'ERP',
          users: '',
          concepts: '',
          businessRules: '',
          constraints: 'Keep the API',
          decisions: 'Do not migrate',
        },
        unmanagedUserFiles: ['context/business.md'],
      }),
    });

    await user.click(await screen.findByRole('button', { name: 'Review' }));

    expect(screen.getByLabelText('What does this system do?')).toHaveValue('ERP');
    expect(screen.getByLabelText(/Decisions to preserve/)).toHaveValue('Do not migrate');
    expect(screen.getByText(/left untouched: context\/business\.md/)).toBeVisible();
  });

  it('goes back to the findings without losing the analysis', async () => {
    const user = userEvent.setup();
    open();

    await user.click(await screen.findByRole('button', { name: 'Review' }));
    await user.click(screen.getByRole('button', { name: 'Back' }));

    expect(await screen.findByRole('heading', { name: 'Analyze Project' })).toBeVisible();
    expect(analyzeProject).toHaveBeenCalledTimes(1);
  });

  it('offers to use or update an existing Harness, with the diff, and never replaces it by default', async () => {
    const user = userEvent.setup();
    const diff = {
      added: [{ id: 'testing:vitest', label: 'Vitest 3', before: null, after: 'Vitest 3' }],
      removed: [],
      changed: [
        { id: 'framework:angular', label: 'Angular 21', before: 'Angular 18', after: 'Angular 21' },
      ],
      unchanged: ['Docker', 'GitHub Actions'],
    };
    open({ analysis: projectAnalysis({ existing: initialized, diff }) });
    vi.mocked(initializeProject).mockResolvedValue(outcome(initialized));

    expect(await screen.findByRole('region', { name: 'Existing Harness detected' })).toBeVisible();
    const diffView = screen.getByRole('region', {
      name: 'Changes compared with the existing Harness',
    });
    expect(within(diffView).getByText('− Angular 18')).toBeVisible();
    expect(within(diffView).getByText('+ Angular 21')).toBeVisible();
    expect(within(diffView).getByText('+ Vitest 3')).toBeVisible();
    expect(within(diffView).getByText('2 unchanged')).toBeVisible();
    expect(screen.queryByRole('button', { name: 'Review' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Use existing' }));

    expect(initializeProject).toHaveBeenCalledWith(
      'w1',
      expect.objectContaining({ mode: 'use_existing' }),
    );
    expect(await screen.findByRole('heading', { name: 'Harness initialized' })).toBeVisible();
  });

  it('updates an existing Harness through the review, starting from the earlier choices', async () => {
    const user = userEvent.setup();
    open({
      analysis: projectAnalysis({
        existing: initialized,
        previousCorrections: { 'framework:angular': '19' },
        previousConfirmed: ['architecture:layered'],
      }),
    });
    vi.mocked(initializeProject).mockResolvedValue(
      outcome(initialized, {
        backedUp: ['context/stack.md', 'project.yaml'],
        leftUntouched: ['context/business.md'],
      }),
    );

    await user.click(await screen.findByRole('button', { name: 'Update existing' }));
    expect(screen.getByLabelText('Value of Angular 21')).toHaveValue('19');
    expect(screen.getByLabelText('Confirm Layered')).toBeChecked();
    await user.click(screen.getByRole('button', { name: 'Initialize' }));

    expect(initializeProject).toHaveBeenCalledWith(
      'w1',
      expect.objectContaining({
        mode: 'update_existing',
        corrections: { 'framework:angular': '19' },
        confirmed: ['architecture:layered'],
      }),
    );
    expect(await screen.findByText(/2 existing files were backed up/)).toBeVisible();
    expect(screen.getByText(/Left untouched: context\/business\.md/)).toBeVisible();
    expect(screen.getByLabelText('Health: Healthy')).toBeVisible();
  });

  it('explains an existing .atlas that is not a valid Harness and repairs it by updating', async () => {
    const user = userEvent.setup();
    open({
      analysis: projectAnalysis({
        existing: harnessSummary({
          status: 'needs_review',
          hasAtlasDir: true,
          problem: 'harness_invalid',
        }),
      }),
    });
    vi.mocked(initializeProject).mockResolvedValue(
      outcome(initialized, { backedUp: ['project.yaml'] }),
    );

    expect(await screen.findByText(/is not a valid Harness/)).toBeVisible();
    await user.click(screen.getByRole('button', { name: 'Review' }));
    await user.click(screen.getByRole('button', { name: 'Initialize' }));

    expect(initializeProject).toHaveBeenCalledWith(
      'w1',
      expect.objectContaining({ mode: 'update_existing' }),
    );
  });

  it('words a failed initialization in the user’s language and keeps the review open', async () => {
    const user = userEvent.setup();
    open();
    vi.mocked(initializeProject).mockRejectedValue({
      code: 'harness_generation_failed',
      params: {},
      detail: 'disk full',
    });

    await user.click(await screen.findByRole('button', { name: 'Review' }));
    await user.click(screen.getByRole('button', { name: 'Initialize' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'The Harness could not be written to .atlas/.',
    );
    expect(screen.getByRole('form', { name: 'Review Project Context' })).toBeVisible();
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Initialize' })).toBeEnabled();
    });
  });

  it('words a failed analysis and offers only to close', async () => {
    mockBackend({ workspaces: [workspace('w1', 'ERP', '/dev/transport-erp')] });
    vi.mocked(analyzeProject).mockRejectedValue({
      code: 'unsafe_project_path',
      params: {},
      detail: null,
    });
    renderWithProviders(
      <InitializeProjectModal workspaceId="w1" onClose={vi.fn()} onInitialized={vi.fn()} />,
    );

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'The project path is not safe to read.',
    );
    expect(screen.queryByRole('button', { name: 'Review' })).not.toBeInTheDocument();
  });

  describe('AI analysis', () => {
    const semanticReport = (extra = {}) => ({
      status: 'completed' as const,
      error: null,
      errorDetail: null,
      sentFiles: [],
      rejected: 0,
      explored: true,
      ...extra,
    });

    it('is recommended, says what the agent will read, and runs only when asked', async () => {
      const user = userEvent.setup();
      open();

      const section = await screen.findByRole('region', { name: 'AI analysis (recommended)' });
      expect(within(section).getByText(/Without AI, Atlas can only list/)).toBeVisible();
      expect(within(section).getByText(/its model provider receives what it reads/)).toBeVisible();
      expect(analyzeProject).toHaveBeenCalledTimes(1);

      vi.mocked(analyzeProject).mockResolvedValue(
        projectAnalysis({
          findings: [...projectAnalysis().findings, modelFinding()],
          analysis: { ...projectAnalysis().analysis, semantic: semanticReport({ rejected: 2 }) },
        }),
      );
      await user.type(
        within(section).getByLabelText('What should the agent know? (optional)'),
        '  ERP for transport companies  ',
      );
      await user.click(within(section).getByRole('button', { name: 'Analyze with AI' }));

      expect(analyzeProject).toHaveBeenLastCalledWith('w1', {
        agentId: 'a1',
        instructions: 'ERP for transport companies',
        restricted: false,
      });
      expect(await screen.findByText(/AI analysis completed/)).toBeVisible();
      expect(screen.getByText(/Atlas cannot list the files/)).toBeVisible();
      expect(screen.getByText(/2 statements were dropped/)).toBeVisible();
      // Nothing was initialized by asking a model.
      expect(initializeProject).not.toHaveBeenCalled();
    });

    it('works with any agent, including OpenCode ones', async () => {
      open({
        agents: [agent('a2', 'Coder', 'opencode', 'opencode/big-pickle')],
      });

      const select = await screen.findByRole('combobox');
      expect(within(select).getByRole('option', { name: 'Coder' })).toBeInTheDocument();
    });

    it('restricted mode lists only agents that can run without tools and shows the files sent', async () => {
      const user = userEvent.setup();
      open({
        agents: [agent('a1', 'Analyst'), agent('a2', 'Coder', 'opencode', 'opencode/big-pickle')],
      });

      await user.click(await screen.findByLabelText(/Restricted mode/));
      const select = screen.getByRole('combobox');
      expect(within(select).getByRole('option', { name: 'Analyst' })).toBeInTheDocument();
      expect(within(select).queryByRole('option', { name: 'Coder' })).not.toBeInTheDocument();
      expect(screen.getByText(/receives only a folder tree/)).toBeVisible();

      vi.mocked(analyzeProject).mockResolvedValue(
        projectAnalysis({
          analysis: {
            ...projectAnalysis().analysis,
            semantic: semanticReport({
              explored: false,
              sentFiles: ['package.json', 'src/application/orders.service.ts'],
            }),
          },
        }),
      );
      await user.click(screen.getByRole('button', { name: 'Analyze with AI' }));

      expect(analyzeProject).toHaveBeenLastCalledWith(
        'w1',
        expect.objectContaining({ agentId: 'a1', restricted: true }),
      );
      expect(await screen.findByText('src/application/orders.service.ts')).toBeVisible();
    });

    it('prefills the empty business fields with the AI’s draft and says it is a draft', async () => {
      const user = userEvent.setup();
      open();
      vi.mocked(analyzeProject).mockResolvedValue(
        projectAnalysis({
          user: {
            purpose: '',
            users: 'My own words',
            concepts: '',
            businessRules: '',
            constraints: '',
            decisions: '',
          },
          suggestedUser: {
            purpose: 'ERP for transport companies',
            users: 'AI guess that must not replace mine',
            concepts: 'CT-e, MDF-e',
            businessRules: '',
            constraints: '',
            decisions: '',
          },
          analysis: { ...projectAnalysis().analysis, semantic: semanticReport() },
        }),
      );
      await user.click(await screen.findByRole('button', { name: 'Analyze with AI' }));
      await user.click(await screen.findByRole('button', { name: 'Review' }));

      expect(screen.getByLabelText(/What does this system do\?/)).toHaveValue(
        'ERP for transport companies',
      );
      expect(screen.getByLabelText('Who uses it?')).toHaveValue('My own words');
      expect(screen.getByLabelText(/most important business concepts/)).toHaveValue('CT-e, MDF-e');
      expect(screen.getAllByText(/Drafted by the AI/)).toHaveLength(2);
    });

    it('says when the model could not be used and keeps the deterministic findings', async () => {
      const user = userEvent.setup();
      open();
      vi.mocked(analyzeProject).mockResolvedValue(
        projectAnalysis({
          analysis: {
            ...projectAnalysis().analysis,
            semantic: semanticReport({
              status: 'failed',
              error: 'semantic_analysis_failed',
              explored: true,
            }),
          },
        }),
      );

      await user.click(await screen.findByRole('button', { name: 'Analyze with AI' }));

      expect(await screen.findByRole('alert')).toHaveTextContent(
        /not the structured findings Atlas asked for/,
      );
      expect(
        within(screen.getByRole('region', { name: 'Detected' })).getByText(/Angular 21/),
      ).toBeVisible();
    });

    it('blocks the screen while the agent works and shows what it is doing', async () => {
      const user = userEvent.setup();
      open();
      const analyze = await screen.findByRole('button', { name: 'Analyze with AI' });
      let finish: (value: ReturnType<typeof projectAnalysis>) => void = () => undefined;
      vi.mocked(analyzeProject).mockReturnValue(
        new Promise((resolve) => {
          finish = resolve;
        }),
      );
      vi.mocked(interruptExecution).mockResolvedValue(undefined);

      await user.click(analyze);

      const screenDialog = await screen.findByRole('dialog', {
        name: 'Analyst is analyzing your project',
      });
      const stop = within(screenDialog).getByRole('button', { name: 'Stop analysis' });
      // Nothing to stop until the agent has started.
      expect(stop).toBeDisabled();
      const progress = (
        kind: 'started' | 'step' | 'tool_started' | 'tool_completed' | 'output',
        text: string,
      ) => {
        act(() => {
          emitHarness({ workspaceId: 'w1', agentId: 'a1', executionId: 'semantic-1', kind, text });
        });
      };
      progress('started', 'Analyst');
      progress('step', 'waiting');
      progress('tool_started', 'Read src/app/app.config.ts');
      progress('tool_completed', 'Read src/app/app.config.ts');
      progress('tool_started', 'Glob src/app/**');
      progress('output', 'The core folder holds ');
      progress('output', 'the API services.');
      // Someone else's analysis is not shown here.
      act(() => {
        emitHarness({
          workspaceId: 'w9',
          agentId: 'a1',
          executionId: 'x',
          kind: 'output',
          text: 'NOPE',
        });
      });

      expect(
        within(screenDialog).getByText('Reading and analyzing', { exact: false }),
      ).toBeVisible();
      expect(await within(screenDialog).findByText('Read src/app/app.config.ts')).toBeVisible();
      expect(within(screenDialog).getByText('Glob src/app/**')).toBeVisible();
      expect(
        within(screenDialog).getByText('The core folder holds the API services.'),
      ).toBeVisible();
      expect(within(screenDialog).queryByText(/NOPE/)).not.toBeInTheDocument();
      expect(within(screenDialog).getByRole('button', { name: 'Stop analysis' })).toBeEnabled();

      await user.click(within(screenDialog).getByRole('button', { name: 'Stop analysis' }));
      expect(interruptExecution).toHaveBeenCalledWith({
        workspaceId: 'w1',
        agentId: 'a1',
        executionId: 'semantic-1',
      });

      // When the analysis returns, the screen goes away.
      act(() => {
        finish(projectAnalysis());
      });
      await waitFor(() => {
        expect(
          screen.queryByRole('dialog', { name: /is analyzing your project/ }),
        ).not.toBeInTheDocument();
      });
    });

    it('always gives the reason when the agent’s analysis fails', async () => {
      const user = userEvent.setup();
      open();
      vi.mocked(analyzeProject).mockResolvedValue(
        projectAnalysis({
          analysis: {
            ...projectAnalysis().analysis,
            semantic: semanticReport({
              status: 'failed',
              error: 'semantic_analysis_failed',
              errorDetail: 'The runtime could not complete the request. [ExecutionFailed("401")]',
            }),
          },
        }),
      );

      await user.click(await screen.findByRole('button', { name: 'Analyze with AI' }));

      const alert = await screen.findByRole('alert');
      await user.click(within(alert).getByText('Technical details'));
      expect(within(alert).getByText(/ExecutionFailed\("401"\)/)).toBeVisible();
    });

    it('also shows the reason when the whole analysis call fails', async () => {
      const user = userEvent.setup();
      open();
      const analyze = await screen.findByRole('button', { name: 'Analyze with AI' });
      vi.mocked(analyzeProject).mockRejectedValue({
        code: 'project_analysis_failed',
        params: {},
        detail: 'permission denied reading /dev/erp',
      });

      await user.click(analyze);

      const alert = await screen.findByRole('alert');
      expect(alert).toHaveTextContent('The project could not be analyzed.');
      await user.click(within(alert).getByText('Technical details'));
      expect(within(alert).getByText('permission denied reading /dev/erp')).toBeVisible();
    });

    it('prefills constraints and decisions drafted by the AI and marks them as drafts', async () => {
      const user = userEvent.setup();
      open();
      vi.mocked(analyzeProject).mockResolvedValue(
        projectAnalysis({
          suggestedUser: {
            purpose: '',
            users: '',
            concepts: '',
            businessRules: '',
            constraints: 'Never touch generated api models.',
            decisions: 'Moving to signals.',
          },
          analysis: { ...projectAnalysis().analysis, semantic: semanticReport() },
        }),
      );
      await user.click(await screen.findByRole('button', { name: 'Analyze with AI' }));
      await user.click(await screen.findByRole('button', { name: 'Review' }));

      expect(screen.getByLabelText(/architectural constraints/)).toHaveValue(
        'Never touch generated api models.',
      );
      expect(screen.getByLabelText(/Decisions to preserve/)).toHaveValue('Moving to signals.');
      expect(screen.getAllByText(/Drafted by the AI/)).toHaveLength(2);
    });

    it('asks to create an agent first when there is none', async () => {
      open({ agents: [] });

      expect(await screen.findByText('Create an agent first to use this.')).toBeVisible();
      expect(screen.queryByRole('button', { name: 'Analyze with AI' })).not.toBeInTheDocument();
    });
  });
});
