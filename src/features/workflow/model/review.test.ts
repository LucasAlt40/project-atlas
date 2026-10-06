import {
  changeSet,
  handoff,
  nodeState,
  passwordRecovery,
  run,
  withCode,
} from '@/test/workflowFixtures';
import type { ValidationEntryDto } from '@/lib/tauri/commands';
import {
  divergence,
  findingsOf,
  formatDuration,
  reviewStateOf,
  severityOf,
  summaryOf,
  timelineOf,
  trailOf,
} from './review';

const w = passwordRecovery();

const at = (executionId: string, startedAt: number, outcome: string | null, n = 1) => ({
  attempt: n,
  iteration: n,
  executionId,
  status: 'completed' as const,
  startedAt,
  completedAt: startedAt + 2_000,
  summary: null,
  outcome,
  failure: null,
});

const verdict = (
  nodeId: string,
  executionId: string,
  status: ValidationEntryDto['status'],
  findings: ValidationEntryDto['findings'] = [],
): ValidationEntryDto => ({ nodeId, executionId, status, outcome: status, summary: '', findings });

const finding = (severity: string, extra = {}) => ({
  severity,
  category: 'Tests',
  title: 'Broken',
  file: 'src/a.ts',
  line: 7,
  description: 'It breaks',
  evidence: '',
  recommendation: 'Fix it',
  ...extra,
});

/** Architect, Developer, QA fails, Bug Fixer, QA passes. */
const looped = () =>
  run(
    w,
    'completed',
    {
      architect: nodeState('completed', { attempts: [at('e1', 1_000, 'approved')] }),
      developer: nodeState('completed', { attempts: [at('e2', 4_000, 'implemented')] }),
      qa: nodeState('completed', {
        attempts: [at('e3', 8_000, 'fail', 1), at('e5', 14_000, 'pass', 2)],
      }),
      'bug-fixer': nodeState('completed', { attempts: [at('e4', 11_000, 'fixed')] }),
    },
    {
      completedAt: 20_000,
      state: {
        ...run(w, 'completed', {}).state,
        validationResults: [
          verdict('qa', 'e3', 'fail', [finding('high'), finding('minor')]),
          verdict('qa', 'e5', 'pass'),
        ],
      },
    },
  );

describe('review: timeline', () => {
  it('lists every attempt in the order they started, with their own status and outcome', () => {
    const steps = timelineOf(looped());

    expect(steps.map((s) => `${s.label}:${s.outcome ?? ''}`)).toEqual([
      'Architect:approved',
      'Developer:implemented',
      'QA:fail',
      'Bug Fixer:fixed',
      'QA:pass',
    ]);
    expect(steps.every((s) => s.status === 'completed')).toBe(true);
    expect(steps[2]?.agentId).toBe('a-qa');
    expect(steps[2]?.durationMs).toBe(2_000);
  });

  it('keeps an execution that failed apart from the outcome an agent declared', () => {
    const failed = run(w, 'failed', {
      architect: nodeState('failed', {
        attempts: [{ ...at('e1', 1_000, null), status: 'failed', failure: 'boom' }],
      }),
    });

    const [step] = timelineOf(failed);

    expect(step?.status).toBe('failed');
    expect(step?.outcome).toBeNull();
    expect(step?.failure).toBe('boom');
  });

  it('has no duration for a step that has not finished', () => {
    const open = run(w, 'cancelled', {
      architect: nodeState('cancelled', {
        attempts: [{ ...at('e1', 1_000, null), status: 'cancelled', completedAt: null }],
      }),
    });

    expect(timelineOf(open)[0]?.durationMs).toBeNull();
  });
});

describe('review: findings', () => {
  it('tells errors, warnings and information apart', () => {
    expect(['critical', 'High', 'error'].map(severityOf)).toEqual(['error', 'error', 'error']);
    expect(['warning', 'medium', 'minor'].map(severityOf)).toEqual([
      'warning',
      'warning',
      'warning',
    ]);
    expect(['info', 'note', 'whatever'].map(severityOf)).toEqual(['info', 'info', 'info']);
  });

  it('carries the file, the line, the agent that found it and its outcome', () => {
    const [first] = findingsOf(looped());

    expect(first).toMatchObject({
      severity: 'error',
      label: 'QA',
      outcome: 'fail',
      executionId: 'e3',
    });
    expect(first?.finding.file).toBe('src/a.ts');
    expect(first?.finding.line).toBe(7);
  });

  it('keeps a finding after a later validation passed, and says so', () => {
    const found = findingsOf(looped());

    expect(found).toHaveLength(2);
    expect(found.every((f) => f.addressed)).toBe(true);
  });

  it('does not call a finding addressed when no later validation passed', () => {
    const open = run(
      w,
      'failed',
      {},
      {
        state: {
          ...run(w, 'failed', {}).state,
          validationResults: [verdict('qa', 'e3', 'fail', [finding('error')])],
        },
      },
    );

    expect(findingsOf(open)[0]?.addressed).toBe(false);
  });

  it('tells the story: validator fails, the fixer works, the validator passes', () => {
    const trail = trailOf(looped());

    expect(trail.map(({ step }) => `${step.label}:${step.outcome ?? ''}`)).toEqual([
      'QA:fail',
      'Bug Fixer:fixed',
      'QA:pass',
    ]);
    expect(trail[0]?.validation?.findings).toHaveLength(2);
    expect(trail[2]?.validation?.status).toBe('pass');
  });

  it('has no trail when nothing failed or was found', () => {
    const clean = run(
      w,
      'completed',
      { qa: nodeState('completed', { attempts: [at('e3', 1, 'pass')] }) },
      {
        state: {
          ...run(w, 'completed', {}).state,
          validationResults: [verdict('qa', 'e3', 'pass')],
        },
      },
    );

    expect(trailOf(clean)).toEqual([]);
    expect(findingsOf(clean)).toEqual([]);
  });
});

describe('review: summary', () => {
  it('counts what really happened and invents nothing', () => {
    const summary = summaryOf(withCode(looped(), 'changes_available'));

    expect(summary).toMatchObject({
      steps: 5,
      agents: 4,
      files: 2,
      additions: 113,
      deletions: 4,
      findings: 2,
      errors: 1,
      warnings: 1,
      durationMs: 19_000,
      hasChangeSet: true,
    });
    expect(summary.outcomes.map((o) => `${o.label} ${o.outcome}`)).toEqual([
      'Architect approved',
      'Developer implemented',
      'QA pass',
      'Bug Fixer fixed',
    ]);
  });

  it('does not know the changes of a run that has no change set', () => {
    expect(summaryOf(looped()).hasChangeSet).toBe(false);
  });
});

describe('review: review status and integration status', () => {
  const state = (status: Parameters<typeof withCode>[1], base = looped()) =>
    reviewStateOf(withCode(base, status, {}, status === 'no_changes' ? null : changeSet()));

  it('is ready for review and not applied while the changes wait in the worktree', () => {
    expect(state('changes_available')).toEqual({
      review: 'review.state.ready',
      integration: 'review.integration.notApplied',
      applied: false,
    });
  });

  it('is reviewed and applied-uncommitted only after Apply', () => {
    expect(state('integrated')).toEqual({
      review: 'review.state.reviewed',
      integration: 'review.integration.applied',
      applied: true,
    });
  });

  it('says a blocked apply is not applied', () => {
    expect(state('conflicts').integration).toBe('review.integration.blocked');
    expect(state('blocked').integration).toBe('review.integration.blocked');
    expect(state('failed').integration).toBe('review.integration.failed');
  });

  it('marks a review of a workflow that did not complete as incomplete', () => {
    const failed = { ...looped(), status: 'failed' as const };

    expect(state('changes_available', failed).review).toBe('review.state.incomplete');
  });

  it('has nothing to review or apply without changes', () => {
    expect(state('no_changes')).toMatchObject({
      review: 'review.state.nothing',
      integration: 'review.integration.nothing',
    });
    expect(reviewStateOf(looped()).integration).toBe('review.integration.none');
  });
});

describe('review: what an agent claimed against what Git saw', () => {
  it('separates the claimed-only files from the detected-only ones', () => {
    const h = handoff('developer', 'qa', {
      changedFiles: changeSet().files,
      reportedFiles: ['src/auth/auth.controller.ts', 'src/ghost.ts'],
    });

    expect(divergence(h)).toEqual({
      claimedOnly: ['src/ghost.ts'],
      detectedOnly: ['src/auth/password-reset.ts'],
    });
  });
});

describe('review: durations', () => {
  it('reads as seconds, minutes and hours', () => {
    expect(formatDuration(4_000)).toBe('4s');
    expect(formatDuration(95_000)).toBe('1m 35s');
    expect(formatDuration(3_900_000)).toBe('1h 5m');
  });
});
