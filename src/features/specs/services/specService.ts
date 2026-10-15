import { invokeCommand } from '@/lib/tauri/commands';
import { listenToEvent } from '@/lib/tauri/events';
import type { UnlistenFn } from '@tauri-apps/api/event';
import type { CliStatusDto, SpecChangedDto } from '@/lib/tauri/commands';
import type {
  FindingStatus,
  ImportCandidate,
  NewSpecRequest,
  SpecAction,
  SpecProgress,
  SpecDecision,
  SpecSummary,
  SpecView,
} from '../types';

/** The UI's only door to the Rust core for specifications (ADR 0035). */

export function listSpecs(workspaceId: string): Promise<SpecSummary[]> {
  return invokeCommand('list_specs', { workspaceId });
}

export function getSpec(specId: string): Promise<SpecView> {
  return invokeCommand('get_spec', { specId });
}

export function createSpec(request: NewSpecRequest): Promise<SpecView> {
  return invokeCommand('create_spec', { request });
}

export async function deleteSpec(specId: string): Promise<void> {
  await invokeCommand('delete_spec', { specId });
}

export function assignSpecAgents(
  specId: string,
  authorAgentId: string,
  reviewerAgentId: string,
): Promise<SpecView> {
  return invokeCommand('assign_spec_agents', { specId, authorAgentId, reviewerAgentId });
}

/** The person confirms the request: from here the first document is written. */
export function startSpec(specId: string): Promise<SpecAction> {
  return invokeCommand('start_spec', { specId });
}

export function generateArtifact(specId: string, artifactId: string): Promise<SpecAction> {
  return invokeCommand('generate_spec_artifact', { specId, artifactId });
}

/** Approve (and go on), ask for changes, or have the document written again. */
export function decideArtifact(
  specId: string,
  artifactId: string,
  expectedRevision: number,
  decision: SpecDecision,
  note: string | null,
): Promise<SpecAction> {
  return invokeCommand('decide_spec_artifact', {
    specId,
    artifactId,
    expectedRevision,
    decision,
    note,
  });
}

export function keepArtifact(
  specId: string,
  artifactId: string,
  expectedRevision: number,
): Promise<SpecView> {
  return invokeCommand('keep_spec_artifact', { specId, artifactId, expectedRevision });
}

export function answerQuestion(
  specId: string,
  questionId: string,
  choice: string | null,
  answer: string | null,
): Promise<SpecAction> {
  return invokeCommand('answer_spec_question', { specId, questionId, choice, answer });
}

export function answerFindings(
  specId: string,
  answers: { findingId: string; answer: string }[],
): Promise<SpecAction> {
  return invokeCommand('answer_spec_findings', { specId, answers });
}

export function resolveFinding(
  specId: string,
  findingId: string,
  status: FindingStatus,
  note: string | null,
): Promise<SpecView> {
  return invokeCommand('resolve_spec_finding', { specId, findingId, status, note });
}

export function runReview(specId: string): Promise<SpecAction> {
  return invokeCommand('run_spec_review', { specId });
}

export function waiveReview(specId: string, note: string): Promise<SpecView> {
  return invokeCommand('waive_spec_review', { specId, note });
}

export function approveSpec(specId: string): Promise<SpecView> {
  return invokeCommand('approve_spec', { specId });
}

export function validateSpec(specId: string): Promise<SpecView> {
  return invokeCommand('validate_spec', { specId });
}

export function publishSpec(specId: string, changeId: string | null): Promise<SpecView> {
  return invokeCommand('publish_spec', { specId, changeId });
}

export function archiveSpec(specId: string): Promise<SpecView> {
  return invokeCommand('archive_spec', { specId });
}

export async function cancelRun(specId: string): Promise<void> {
  await invokeCommand('cancel_spec_run', { specId });
}

export function listImportable(workspaceId: string, projectId: string): Promise<ImportCandidate[]> {
  return invokeCommand('list_importable_specs', { workspaceId, projectId });
}

export function importSpec(
  workspaceId: string,
  projectId: string,
  changeId: string,
  authorAgentId: string,
  reviewerAgentId: string | null,
): Promise<SpecView> {
  return invokeCommand('import_spec', {
    workspaceId,
    projectId,
    changeId,
    authorAgentId,
    reviewerAgentId,
  });
}

/** How far each run started from the spec has got. */
export function getSpecProgress(specId: string): Promise<SpecProgress[]> {
  return invokeCommand('get_spec_progress', { specId });
}

export function getCliStatus(specId: string): Promise<CliStatusDto> {
  return invokeCommand('get_openspec_cli_status', { specId });
}

export function checkWithCli(specId: string): Promise<SpecView> {
  return invokeCommand('check_spec_with_cli', { specId });
}

export function archiveWithCli(specId: string): Promise<SpecView> {
  return invokeCommand('archive_spec_with_cli', { specId });
}

export function listReadySpecs(workspaceId: string): Promise<SpecSummary[]> {
  return invokeCommand('list_ready_specs', { workspaceId });
}

/** A spec changed (any part of it): read it again. */
export function subscribeToSpecChanges(
  handler: (change: SpecChangedDto) => void,
): Promise<UnlistenFn> {
  return listenToEvent('spec:changed', handler);
}

/** The person says to go on after the spec's token budget was reached. */
export function raiseBudget(specId: string): Promise<SpecView> {
  return invokeCommand('raise_spec_budget', { specId });
}

/** The person chooses which of the workspace's integrations the spec is tested and reviewed with. */
export function setSpecIntegrations(specId: string, connectionIds: string[]): Promise<SpecView> {
  return invokeCommand('set_spec_integrations', { specId, connectionIds });
}
