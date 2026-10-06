import type {
  ContextRecordDto,
  ContextReviewDto,
  GuardrailMetricsDto,
  InteractionDetectionDto,
  UsageMetricsDto,
} from '@/lib/tauri/commands';
import type { StoredExecution } from '../types';
import { toActivityEntry } from './activity';
import type { AgentRun, ProcessState } from './agentRuns';

/** `#12` for `exec-12`: short enough to say aloud, still unique in the app. */
export function shortId(executionId: string): string {
  return `#${executionId.replace(/^exec-/, '')}`;
}

/** An ended execution shaped like a live run, so the activity view can show either. */
export function runFromStored(execution: StoredExecution): AgentRun {
  return {
    executionId: execution.id,
    status: execution.status,
    startedAt: execution.startedAt,
    activity: execution.events.map((event, index) => toActivityEntry(event, index)),
    failureKind: execution.failure?.kind ?? null,
    receiving: false,
    pendingApprovalId: null,
  };
}

/** What the user did to the process, if anything (it is what makes a run "cancelled"). */
export type StoppedBy = 'interrupted' | 'terminated';

function stoppedByOf(execution: StoredExecution): StoppedBy | null {
  const kinds = execution.events.map((event) => event.kind);
  if (kinds.includes('user_terminated')) return 'terminated';
  if (kinds.includes('user_interrupted')) return 'interrupted';
  return null;
}

/** The facts the inspector shows about one execution, live or ended. Nothing is invented. */
export interface ExecutionFacts {
  executionId: string;
  status: AgentRun['status'];
  /** What the user asked, when known. */
  task: string | null;
  startedAt: number;
  endedAt: number | null;
  runtimeId: string;
  modelId: string;
  failureKind: string | null;
  /** Runtime-reported usage: `null` is "not reported", not zero. */
  usage: UsageMetricsDto | null;
  /** The run has not ended, so its usage cannot be known yet. */
  usagePending: boolean;
  process: 'running' | 'stopping' | 'exited' | 'none';
  exitCode: string | null;
  stoppedBy: StoppedBy | null;
  /** How the Harness context of its prompt was chosen (absent without a Harness). */
  context: ContextRecordDto | null;
  /** What it asked, when it is waiting for a person. */
  interaction: InteractionDetectionDto | null;
  /** The review of the context it was given and what the guardrails decided (absent when off). */
  contextReview: ContextReviewDto | null;
  guardrails: GuardrailMetricsDto | null;
}

export function factsFromStored(execution: StoredExecution): ExecutionFacts {
  const exited = execution.events.find((event) => event.kind === 'process_exited');
  const hadTerminal = execution.events.some((event) => event.kind === 'terminal_connected');
  return {
    executionId: execution.id,
    status: execution.status,
    task: execution.task,
    startedAt: execution.startedAt,
    endedAt: execution.completedAt,
    runtimeId: execution.runtimeId,
    modelId: execution.modelId,
    failureKind: execution.failure?.kind ?? null,
    usage: execution.usage,
    usagePending: false,
    process: exited || hadTerminal ? 'exited' : 'none',
    exitCode: exited?.metadata.exitCode ?? null,
    stoppedBy: stoppedByOf(execution),
    context: execution.context ?? null,
    interaction: execution.interaction ?? null,
    contextReview: execution.optimization?.contextReview ?? null,
    guardrails: execution.optimization?.guardrails ?? null,
  };
}

/** A run that is still going (or has just ended and is not stored yet). */
export function factsFromRun(
  run: AgentRun,
  process: ProcessState | undefined,
  agent: { runtimeId: string; modelId: string },
  task: string | null,
): ExecutionFacts {
  const processFact = !process
    ? 'none'
    : process.status === 'exited'
      ? 'exited'
      : process.status === 'running'
        ? 'running'
        : 'stopping';
  return {
    executionId: run.executionId,
    status: run.status,
    task,
    startedAt: run.startedAt,
    endedAt: null,
    runtimeId: agent.runtimeId,
    modelId: agent.modelId,
    failureKind: run.failureKind,
    usage: null,
    usagePending: run.status === 'running',
    process: processFact,
    exitCode: process?.exitCode === null || process === undefined ? null : String(process.exitCode),
    stoppedBy:
      process?.userAction === 'interrupted' || process?.userAction === 'terminated'
        ? process.userAction
        : null,
    context: null,
    interaction: null,
    contextReview: null,
    guardrails: null,
  };
}

/** Splits executions (newest first) into those of the local calendar day of `now` and older. */
export function splitByDay(
  executions: StoredExecution[],
  now: number,
): { today: StoredExecution[]; earlier: StoredExecution[] } {
  const midnight = new Date(now);
  midnight.setHours(0, 0, 0, 0);
  const start = midnight.getTime();
  return {
    today: executions.filter((e) => e.startedAt >= start),
    earlier: executions.filter((e) => e.startedAt < start),
  };
}
