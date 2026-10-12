/**
 * What the phone's page reads from the Atlas server (`src-tauri/src/application/remote/view.rs`).
 * These are a projection built for a small screen, not the desktop's own types: no folders, no
 * raw output, and only the progress the engine can vouch for.
 */

export type RunStatus =
  'running' | 'paused' | 'waiting_for_input' | 'completed' | 'failed' | 'cancelled' | 'interrupted';

export type NodeStatus =
  | 'pending'
  | 'ready'
  | 'running'
  | 'waiting_approval'
  | 'waiting_for_input'
  | 'completed'
  | 'failed'
  | 'blocked'
  | 'cancelled'
  | 'skipped';

export type AttemptStatus =
  'running' | 'completed' | 'failed' | 'cancelled' | 'interrupted' | 'waiting_for_input';

export type Attention = 'none' | 'needs_answer' | 'needs_computer' | 'failed';

export type InteractionKind =
  'clarification' | 'approval' | 'permission' | 'runtime_confirmation' | 'plan';

export type InteractionStatus = 'pending' | 'answered' | 'cancelled' | 'expired';

export interface StepBrief {
  nodeId: string;
  label: string;
  status: NodeStatus;
  agent: string | null;
}

export interface FailureView {
  code: string;
  step: string | null;
  detail: string | null;
}

export interface RunSummary {
  id: string;
  workflowName: string;
  workspaceId: string;
  workspaceName: string;
  task: string;
  status: RunStatus;
  attention: Attention;
  pending: number;
  current: StepBrief[];
  stepsTotal: number;
  stepsDone: number;
  startedAt: number;
  updatedAt: number;
  completedAt: number | null;
  failure: FailureView | null;
}

export interface AttemptView {
  attempt: number;
  iteration: number;
  status: AttemptStatus;
  startedAt: number;
  completedAt: number | null;
  summary: string | null;
  outcome: string | null;
  failure: string | null;
}

export interface StepView {
  nodeId: string;
  kind: 'agent' | 'condition' | 'end';
  label: string;
  status: NodeStatus;
  agent: string | null;
  iterations: number;
  reason: string | null;
  startedAt: number | null;
  completedAt: number | null;
  attempts: AttemptView[];
}

export interface EdgeView {
  from: string;
  to: string;
  label: string;
  conditional: boolean;
}

export interface InteractionOption {
  id: string;
  label: string;
}

/** What the phone may do about a question. */
export interface RemoteRule {
  canAnswer: boolean;
  /** The option ids the phone may send; `null` means any of the question's options and free text. */
  choices: string[] | null;
  desktopOnly: 'guardrail' | 'grants_authority' | null;
}

export interface InteractionView {
  id: string;
  nodeId: string;
  step: string;
  agent: string | null;
  kind: InteractionKind;
  status: InteractionStatus;
  question: string;
  context: string;
  document: string;
  options: InteractionOption[];
  createdAt: number;
  answeredAt: number | null;
  choice: string | null;
  answer: string | null;
  auto: boolean;
  remote: RemoteRule;
}

export interface EventView {
  kind: string;
  step: string | null;
  message: string;
  at: number;
}

export interface ResultView {
  summary: string | null;
  artifacts: { name: string; kind: string; summary: string }[];
  decisions: { title: string; decision: string }[];
}

export interface CodeView {
  status: string;
  filesChanged: number;
  additions: number;
  deletions: number;
}

export interface Controls {
  pause: boolean;
  resume: boolean;
  cancel: boolean;
}

export interface RunDetail extends RunSummary {
  steps: StepView[];
  edges: EdgeView[];
  interactions: InteractionView[];
  events: EventView[];
  result: ResultView;
  code: CodeView;
  autoApprovePlans: boolean;
  controls: Controls;
}

export interface Overview {
  serverTime: number;
  language: string;
  runs: RunSummary[];
}

export interface SessionInfo {
  device: { id: string; name: string };
  serverTime: number;
  epoch: string;
  language?: string;
}

/** How the backend settled an answer. */
export type Applied = 'now' | 'already' | 'pending' | 'recorded';

export interface Answered {
  applied: Applied;
  run: RunDetail;
}

/** One hint from the event stream. It says that something changed, never what. */
export interface ChangeEvent {
  seq: number;
  kind: string;
  runId: string | null;
  workspaceId: string | null;
  at: number;
}
