import type {
  SpecArtifactDto,
  SpecArtifactIdDto,
  SpecArtifactPhaseDto,
  SpecDto,
  SpecFileDto,
  SpecRevisionDto,
  SpecStatusDto,
  SpecSummaryDto,
  SpecValidationIssueDto,
  SpecViewDto,
} from '@/lib/tauri/commands';

const NOW = Date.now();
const ORDER: SpecArtifactIdDto[] = ['proposal', 'specs', 'design', 'tasks'];
const DEPENDS: Record<SpecArtifactIdDto, SpecArtifactIdDto[]> = {
  proposal: [],
  specs: ['proposal'],
  design: ['proposal', 'specs'],
  tasks: ['specs', 'design'],
};

export const PROPOSAL_TEXT =
  '# Proposal\n\n## Why\n\nNight users find the bright interface tiring and ask for a dark theme.\n\n## What Changes\n\n- Add a dark theme switch.\n';

export const SPEC_TEXT =
  '## ADDED Requirements\n\n### Requirement: User can switch the theme\nThe system SHALL let the user pick light or dark.\n\n#### Scenario: Switch to dark\n- **WHEN** the user picks dark\n- **THEN** the interface uses the dark theme\n';

export function file(path: string, content: string): SpecFileDto {
  return { path, content };
}

/** A document that has one revision, and what is said about it. */
export interface Written {
  files?: SpecFileDto[];
  review?: SpecArtifactDto['review'];
  approved?: boolean;
  issues?: SpecValidationIssueDto[];
  generation?: SpecArtifactDto['generation'];
  outdated?: boolean;
  revisions?: number;
}

export function issue(
  code: string,
  severity: SpecValidationIssueDto['severity'] = 'error',
  params: Record<string, string> = {},
): SpecValidationIssueDto {
  return { code, severity, file: null, line: null, message: code, params };
}

export function artifact(id: SpecArtifactIdDto, written?: Written): SpecArtifactDto {
  const count = written ? (written.revisions ?? 1) : 0;
  const files = written?.files ?? [
    file(
      id === 'specs' ? 'specs/dark-mode/spec.md' : `${id}.md`,
      id === 'specs'
        ? SPEC_TEXT
        : id === 'proposal'
          ? PROPOSAL_TEXT
          : `# ${id}\n\nText of the ${id}.`,
    ),
  ];
  const revisions = Array.from({ length: count }, (_, index): SpecRevisionDto => ({
    number: index + 1,
    files,
    origin: index === 0 ? 'generated' : 'revised',
    runId: `exec-${id}-${String(index + 1)}`,
    agentId: 'a1',
    basedOn: {},
    createdAt: NOW - 60_000,
  }));
  const approved = written?.approved === true;
  return {
    id,
    kind: id,
    dependsOn: DEPENDS[id],
    revisions,
    generation: written?.generation ?? { state: 'idle' },
    review: written ? (written.review ?? (approved ? 'approved' : 'awaiting_review')) : 'none',
    approvedRevision: approved ? count : null,
    outdated: written?.outdated ? { because: 'specs', revision: 2, at: NOW } : null,
    validation: written
      ? { revision: count, validator: 'atlas-openspec/1', issues: written.issues ?? [], at: NOW }
      : null,
    feedback: [],
    agentId: written ? 'a1' : null,
    createdAt: NOW - 120_000,
    updatedAt: NOW - 30_000,
  };
}

export function spec(patch: Partial<SpecDto> = {}): SpecDto {
  return {
    id: 'spec-1',
    title: 'Dark mode',
    request: 'Add a dark mode to the settings screen',
    context: '',
    workspaceId: 'ws1',
    projectId: 'ws1-main',
    framework: 'openspec',
    frameworkSchema: 'spec-driven/1',
    changeId: 'dark-mode',
    authorAgentId: 'a1',
    reviewerAgentId: 'a1',
    started: true,
    artifacts: ORDER.map((id) => artifact(id)),
    questions: [],
    findings: [],
    aiReview: null,
    consistency: null,
    finalApproval: null,
    publication: { state: 'not_published' },
    archived: null,
    cliCheck: null,
    implementations: [],
    runs: [],
    budgetSteps: 0,
    integrations: [],
    origin: { kind: 'created' },
    version: 1,
    createdAt: NOW - 200_000,
    updatedAt: NOW - 30_000,
    ...patch,
  };
}

function phaseOf(a: SpecArtifactDto, all: SpecArtifactDto[]): SpecArtifactPhaseDto {
  if (a.generation.state === 'running') return 'generating';
  if (a.generation.state === 'waiting_for_input') return 'waiting_for_input';
  if (a.generation.state === 'failed') return 'failed';
  if (a.revisions.length === 0) {
    const ready = a.dependsOn.every((d) => all.find((x) => x.id === d)?.review === 'approved');
    return ready ? 'ready' : 'blocked';
  }
  if (a.outdated) return 'outdated';
  if (a.review === 'changes_requested') return 'changes_requested';
  return a.review === 'approved' ? 'approved' : 'awaiting_review';
}

/** A view of the spec with the phases worked out from its documents. */
export function view(
  status: SpecStatusDto,
  specPatch: Partial<SpecDto> = {},
  patch: Partial<SpecViewDto> = {},
): SpecViewDto {
  const s = spec(specPatch);
  const phases = Object.fromEntries(s.artifacts.map((a) => [a.id, phaseOf(a, s.artifacts)]));
  const current = Object.fromEntries(
    s.artifacts.filter((a) => a.revisions.length > 0).map((a) => [a.id, a.revisions.length]),
  );
  const ready = s.artifacts.filter((a) => phases[a.id] === 'ready').map((a) => a.id);
  return {
    spec: s,
    status,
    phases,
    readiness: { ready: false, blockers: [] },
    currentRevisions: current,
    readyToGenerate: s.started ? ready : [],
    reviewCurrent: false,
    consistencyCurrent: false,
    finalApprovalCurrent: false,
    publicationCurrent: false,
    busy: s.artifacts.some((a) => a.generation.state === 'running'),
    pendingQuestions: s.questions.filter((q) => q.status === 'pending').length,
    openFindings: s.findings.filter((f) => f.status === 'open').length,
    tasks: [],
    outline: [],
    projectAvailable: true,
    authorAvailable: true,
    reviewerAvailable: true,
    usage: {
      runs: [],
      inputTokens: null,
      outputTokens: null,
      cachedInputTokens: null,
      cost: null,
      currency: null,
      runsTotal: 0,
      runsMeasured: 0,
    },
    budget: { limit: null, spent: null, steps: 0, reached: false, near: false },
    integrations: [],
    ...patch,
  };
}

export function summary(patch: Partial<SpecSummaryDto> = {}): SpecSummaryDto {
  return {
    id: 'spec-1',
    title: 'Dark mode',
    request: 'Add a dark mode to the settings screen',
    workspaceId: 'ws1',
    projectId: 'ws1-main',
    changeId: 'dark-mode',
    status: 'awaiting_review',
    approved: 1,
    stageTotal: 4,
    currentArtifact: 'specs',
    pendingQuestions: 0,
    openFindings: 0,
    authorAgentId: 'a1',
    implementations: 0,
    imported: false,
    createdAt: NOW - 200_000,
    updatedAt: NOW - 30_000,
    ...patch,
  };
}

/** Everything written and approved, each document valid. */
export function allApproved(): SpecDto['artifacts'] {
  return ORDER.map((id) => artifact(id, { approved: true }));
}
