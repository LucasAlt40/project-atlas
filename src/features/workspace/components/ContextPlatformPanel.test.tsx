import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { I18nProvider } from '@/i18n/I18nProvider';
import type {
  ContextManifestDto,
  ContextPlanDto,
  FigureDto,
  ManifestRuleDto,
  OptimizationMetricsDto,
} from '@/lib/tauri/commands';
import { storedExecution } from '@/test/fixtures';
import type { StoredExecution } from '../types';
import { ContextPlatformPanel } from './ContextPlatformPanel';
import { ExecutionInspector } from './ExecutionInspector';

const unknown: FigureDto = { value: null, source: 'unknown', precision: 'unknown' };
const estimated = (value: number): FigureDto => ({
  value,
  source: 'default',
  precision: 'estimated',
  method: 'chars_div_4',
});

function metrics(): OptimizationMetricsDto {
  return {
    prompt: { totalBytes: 16_840, estimatedTokens: 4_210, tokenSource: 'estimated', sections: [] },
    context: null,
    latency: {
      contextBuildMs: null,
      promptBuildMs: null,
      runtimeStartupMs: null,
      runtimeExecutionMs: null,
      runtimeMs: null,
      totalMs: null,
      instrumentationMs: null,
    },
    tools: { calls: null, totalOutputBytes: null },
    handoff: { bytes: null },
    optimization: {
      cacheHits: null,
      cacheMisses: null,
      deduplicatedItems: 0,
      compressedItems: 0,
      droppedItems: null,
    },
    budget: {
      runtimeId: 'claude',
      modelId: 'sonnet',
      limits: { input: unknown, output: unknown, total: unknown },
      outputReserve: unknown,
      safetyMargin: unknown,
      input: { limit: unknown, used: estimated(4_210), remaining: unknown },
      resolution: [],
    },
  };
}

const PLAN: ContextPlanDto = {
  sections: [
    { section: 'task_context', bytes: 7_200, tokens: estimated(1_800) },
    { section: 'task', bytes: 2_400, tokens: estimated(600) },
  ],
  totalBytes: 16_840,
  totalTokens: estimated(4_210),
  fingerprint: 'sha256:plan',
  engineOmittedItems: 0,
};

const RULES: ManifestRuleDto[] = [
  {
    reference: 'project.tests',
    title: 'Testing required',
    scope: 'project',
    strength: 'mandatory',
    authority: 'authoritative',
    origin: 'project_file',
    source: '.atlas/context/rules.md',
    status: 'applied',
    inConflict: true,
    downgraded: false,
  },
  {
    reference: 'task.skip',
    title: 'Skip the tests',
    scope: 'task',
    strength: 'preference',
    authority: 'authoritative',
    origin: 'user',
    source: 'config',
    status: 'overridden',
    by: 'project.tests',
    inConflict: true,
    downgraded: false,
  },
  {
    reference: 'global.note',
    title: 'The billing module is legacy',
    scope: 'global',
    strength: 'informational',
    authority: 'informational',
    origin: 'external',
    source: 'https://example.test',
    status: 'applied',
    inConflict: false,
    downgraded: true,
  },
];

function manifest(overrides: Partial<ContextManifestDto> = {}): ContextManifestDto {
  return {
    executionId: 'exec-1',
    workspaceId: 'w1',
    taskId: 't1',
    agentId: 'a1',
    runtimeId: 'claude',
    modelId: 'sonnet',
    createdAt: 1,
    planFingerprint: 'sha256:plan',
    divergedFromPlan: false,
    sections: [],
    delivery: {
      delivered: true,
      promptHash: 'sha256:abc123',
      bytes: 16_840,
      chars: 16_800,
      estimatedTokens: estimated(4_210),
      systemChannel: 'unsupported',
      systemBytes: 0,
    },
    rules: RULES,
    surface: {
      runtimeId: 'claude',
      entries: [
        {
          kind: 'prompt',
          control: 'atlas_controlled',
          observation: 'declared',
          reachesModel: true,
        },
        {
          kind: 'system_channel',
          control: 'atlas_controlled',
          observation: 'declared',
          reachesModel: false,
          detail: 'prompt_body',
        },
        {
          kind: 'tools',
          control: 'atlas_controlled',
          observation: 'reported',
          reachesModel: false,
          detail: 'Glob,Grep,Read',
        },
        {
          kind: 'plugins',
          control: 'user_controlled',
          observation: 'reported',
          reachesModel: false,
          detail: 'figma@claude-plugins-official',
        },
        {
          kind: 'user_instructions',
          control: 'user_controlled',
          observation: 'not_observed',
          reachesModel: false,
        },
        { kind: 'other', control: 'unknown', observation: 'not_observed', reachesModel: false },
      ],
    },
    warnings: ['tokens_estimated', 'model_limit_unknown', 'surface_partly_unobserved'],
    ...overrides,
  };
}

/** A stored execution with the context record Atlas keeps for every delivery. */
function execution(
  options: { metrics?: boolean; manifest?: ContextManifestDto } = {},
): StoredExecution {
  return {
    ...storedExecution('w1', 'a1', 'exec-1'),
    ...(options.metrics === false ? {} : { optimization: metrics() }),
    plan: PLAN,
    manifest: options.manifest ?? manifest(),
  };
}

function show(language: 'en-US' | 'pt-BR', value: StoredExecution) {
  render(
    <I18nProvider language={language}>
      <ContextPlatformPanel execution={value} />
    </I18nProvider>,
  );
}

describe('ContextPlatformPanel', () => {
  it('shows an estimate as an estimate and a limit nobody stated as unknown', () => {
    show('en-US', execution());

    const budget = screen.getByRole('region', { name: 'Budget' });
    expect(within(budget).getByText(/~4\.2K/)).toBeInTheDocument();
    expect(
      within(budget).getByText(/Atlas default · estimated · characters ÷ 4/),
    ).toBeInTheDocument();
    expect(within(budget).getByText('Input limit').nextElementSibling).toHaveTextContent('unknown');
  });

  it('still shows the plan and the manifest when metrics were off (no budget then)', () => {
    show('en-US', execution({ metrics: false }));

    expect(screen.queryByRole('region', { name: 'Budget' })).not.toBeInTheDocument();
    expect(screen.getByText('sha256:abc123')).toBeInTheDocument();
  });

  it('shows the plan, and the manifest with the hash of what was delivered', () => {
    show('en-US', execution());

    const plan = screen.getByRole('region', { name: 'Plan: what Atlas meant to send' });
    expect(within(plan).getByText(/Task context: ~1\.8K/)).toBeInTheDocument();
    const delivered = screen.getByRole('region', { name: /^Manifest/ });
    expect(within(delivered).getByText('sha256:abc123')).toBeInTheDocument();
    expect(within(delivered).getByText('same prompt as planned')).toBeInTheDocument();
    expect(within(delivered).getByText('yes')).toBeInTheDocument();
    expect(within(delivered).getByText('in the prompt body')).toBeInTheDocument();
  });

  it('says how the system instructions travelled when a runtime has a channel', () => {
    const value = manifest();
    value.delivery.systemChannel = 'native';
    value.delivery.systemBytes = 1_200;

    show('en-US', execution({ manifest: value }));

    expect(screen.getByText(/on the native system channel \(1,200 bytes\)/)).toBeInTheDocument();
  });

  it('groups the surface by who controls it and confirms only the delivered prompt', () => {
    show('en-US', execution());

    const surface = screen.getByRole('region', { name: 'Runtime surface' });
    for (const heading of ['Atlas controls', 'Your setup controls', 'Unknown']) {
      expect(within(surface).getByRole('heading', { name: heading })).toBeInTheDocument();
    }
    expect(within(surface).getByText('figma@claude-plugins-official')).toBeInTheDocument();
    expect(within(surface).getAllByText(/delivered to the model/)).toHaveLength(1);
    expect(within(surface).getByText(/Instruction files/)).toHaveTextContent('not observed');
    // The system channel is declared in words, not as a code.
    expect(
      within(surface).getByText('in the prompt body (no system channel is used)'),
    ).toBeInTheDocument();
  });

  it('lists the warnings', () => {
    show('en-US', execution());

    const warnings = screen.getByRole('region', { name: 'Warnings' });
    expect(
      within(warnings).getByText('Token counts are Atlas estimates, not counts by the model.'),
    ).toBeInTheDocument();
    expect(
      within(warnings).getByText("Nothing states this model's limit for this runtime."),
    ).toBeInTheDocument();
  });

  it('says a step held back by the guardrails was not delivered', () => {
    const value = manifest();
    value.delivery.delivered = false;

    show('en-US', execution({ manifest: value }));

    expect(screen.getByText('no, held back before it started')).toBeInTheDocument();
  });

  it('is translated: the same facts in Portuguese', () => {
    show('pt-BR', execution());

    expect(screen.getByRole('region', { name: 'Orçamento' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'O Atlas controla' })).toBeInTheDocument();
    expect(screen.getByRole('region', { name: 'Regras' })).toBeInTheDocument();
    expect(screen.getAllByText(/desconhecido/, { selector: 'dd' }).length).toBeGreaterThan(0);
  });
});

describe('Rules in the context tab', () => {
  it('lists the rules by scope with their strength, authority, origin and status', () => {
    show('en-US', execution());

    const rules = screen.getByRole('region', { name: 'Rules' });
    for (const heading of ['Global', 'Project', 'Task']) {
      expect(within(rules).getByRole('heading', { name: heading })).toBeInTheDocument();
    }
    const project = within(rules).getByText('Testing required').closest('li');
    expect(project).toHaveTextContent('mandatory · binds the agent · project file');
    expect(project).toHaveTextContent('.atlas/context/rules.md');
    expect(project).toHaveTextContent('✓');
  });

  it('says why a rule did not reach the agent, and by which rule it was overridden', () => {
    show('en-US', execution());

    const skipped = screen.getByText('Skip the tests').closest('li');
    expect(skipped).toHaveTextContent('✗');
    expect(skipped).toHaveTextContent('overridden on its topic by project.tests');
  });

  it('flags a conflict and a rule kept as background for its origin', () => {
    show('en-US', execution());

    const rules = screen.getByRole('region', { name: 'Rules' });
    expect(within(rules).getAllByText(/conflict settled/)).toHaveLength(2);
    const note = within(rules).getByText('The billing module is legacy').closest('li');
    expect(note).toHaveTextContent('kept as background: its source cannot bind');
    expect(note).toHaveTextContent('background only');
  });

  it('says what a mandatory rule is checked against, and that the rest is left to the model', () => {
    show('en-US', execution());

    expect(
      screen.getByText(/checked against the task and the other instructions/),
    ).toBeInTheDocument();
    expect(screen.getByText(/left to the model/)).toBeInTheDocument();
  });

  it('says plainly that a rule grants no permission', () => {
    show('en-US', execution());

    expect(screen.getByText(/They grant no permission/)).toBeInTheDocument();
  });

  it('says so when no rule applied', () => {
    const value = manifest();
    value.rules = [];

    show('en-US', execution({ manifest: value }));

    expect(screen.getByText('No rule applied to this execution.')).toBeInTheDocument();
  });
});

describe('MCP in the context tab', () => {
  const record = {
    servers: [
      {
        connectionId: 'c1',
        name: 'files',
        enabled: true,
        required: false,
        authorized: true,
        exposed: true,
        discoveredStatus: 'connected' as const,
        reportedStatus: 'connected' as const,
      },
      {
        connectionId: 'c2',
        name: 'figma',
        enabled: true,
        required: true,
        authorized: true,
        exposed: false,
        problem: { kind: 'secret_missing' as const, name: 'FIGMA_TOKEN' },
      },
      {
        connectionId: 'c3',
        name: 'idle',
        enabled: true,
        required: false,
        authorized: false,
        exposed: false,
      },
    ],
    tools: [
      {
        server: 'files',
        tool: 'read',
        discovered: true,
        enabled: true,
        authorized: true,
        exposed: true,
        reportedExposed: true,
        used: null,
      },
      {
        server: 'files',
        tool: 'write',
        discovered: true,
        enabled: true,
        authorized: false,
        exposed: false,
        reportedExposed: false,
        used: false,
      },
    ],
    heldBack: ['files/write'],
    unauthorized: [] as string[],
  };

  function withMcp(mcp: typeof record) {
    const value = manifest();
    value.mcp = mcp;
    return execution({ manifest: value });
  }

  it('keeps the six facts about a tool apart and says "not reported" instead of no', () => {
    show('en-US', withMcp(record));

    const panel = screen.getByRole('region', { name: 'MCP' });
    const read = within(panel).getByText('read').closest('tr');
    expect(read).not.toBeNull();
    const cell = (label: string) =>
      within(read as HTMLElement).getByLabelText(new RegExp(`^${label}:`));
    expect(cell('Discovered')).toHaveAttribute('aria-label', 'Discovered: yes');
    expect(cell('Authorized')).toHaveAttribute('aria-label', 'Authorized: yes');
    expect(cell('Exposed')).toHaveAttribute('aria-label', 'Exposed: yes');
    // The runtime listed it; whether the model called it was not reported.
    expect(cell('Listed by runtime')).toHaveAttribute('aria-label', 'Listed by runtime: yes');
    expect(cell('Used')).toHaveAttribute('aria-label', 'Used: not reported');
    // The one held back: discovered and enabled, but not authorized and not exposed.
    const write = within(panel).getByText('write').closest('tr') as HTMLElement;
    expect(within(write).getByLabelText(/^Authorized:/)).toHaveAttribute(
      'aria-label',
      'Authorized: no',
    );
    expect(within(panel).getByText(/Held back from the runtime: files\/write/)).toBeInTheDocument();
  });

  it('says why a connection was not given, and that a granted one that is not enabled is not exposed', () => {
    show('en-US', withMcp(record));

    const panel = screen.getByRole('region', { name: 'MCP' });
    expect(
      within(panel).getByText('⚠ Not given: a secret it needs is not stored.'),
    ).toBeInTheDocument();
    expect(within(panel).getByText('FIGMA_TOKEN')).toBeInTheDocument();
    expect(
      within(panel).getByText(/required · authorized for this step · not given/),
    ).toBeInTheDocument();
    expect(within(panel).getByText(/not authorized for this step/)).toBeInTheDocument();
  });

  it('raises an alert when the runtime listed tools nobody authorized', () => {
    show('en-US', withMcp({ ...record, unauthorized: ['files/extra', 'mcp__other__x'] }));

    expect(screen.getByRole('alert')).toHaveTextContent(
      'the step was stopped: files/extra, mcp__other__x',
    );
  });

  it('shows nothing for a workspace with no connection', () => {
    show('en-US', withMcp({ ...record, servers: [], tools: [], heldBack: [] }));

    expect(screen.queryByRole('region', { name: 'MCP' })).not.toBeInTheDocument();
  });

  it('is translated', () => {
    show('pt-BR', withMcp(record));

    const panel = screen.getByRole('region', { name: 'MCP' });
    expect(within(panel).getByText(/Não entregue: um segredo/)).toBeInTheDocument();
    expect(within(panel).getAllByText('Listada pelo runtime').length).toBeGreaterThan(0);
  });
});

describe('Execution Inspector context tab', () => {
  const context = {
    workspaceName: 'ERP',
    agentName: 'Architect',
    personalityName: 'Architect',
    runtimeName: 'Claude Code',
    capabilities: undefined,
    worktreeIsolation: true,
  };

  function open(stored: StoredExecution) {
    render(
      <I18nProvider language="en-US">
        <ExecutionInspector execution={stored} messages={[]} context={context} onClose={vi.fn()} />
      </I18nProvider>,
    );
  }

  it('shows the context record of an execution that has one', async () => {
    open(execution());

    await userEvent.click(screen.getByRole('tab', { name: 'Context' }));

    expect(screen.getByText('sha256:abc123')).toBeInTheDocument();
  });

  it('says so when there is no record, instead of an empty tab', async () => {
    open(storedExecution('w1', 'a1', 'exec-9'));

    await userEvent.click(screen.getByRole('tab', { name: 'Context' }));

    expect(screen.getByText(/No context record for this execution/)).toBeInTheDocument();
  });
});
