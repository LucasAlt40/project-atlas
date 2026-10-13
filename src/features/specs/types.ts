import type {
  ImportCandidateDto,
  NewSpecRequestDto,
  SpecActionDto,
  SpecArtifactDto,
  SpecArtifactPhaseDto,
  SpecDecisionDto,
  SpecFindingDto,
  SpecFindingStatusDto,
  SpecQuestionDto,
  SpecProgressDto,
  SpecStatusDto,
  SpecSummaryDto,
  SpecValidationIssueDto,
  SpecViewDto,
} from '@/lib/tauri/commands';

// The wire types are the domain types for now; aliased so the feature does not import DTOs by name.
export type SpecView = SpecViewDto;
export type SpecSummary = SpecSummaryDto;
export type SpecArtifact = SpecArtifactDto;
export type SpecStatus = SpecStatusDto;
export type ArtifactPhase = SpecArtifactPhaseDto;
export type SpecQuestion = SpecQuestionDto;
export type SpecFinding = SpecFindingDto;
export type FindingStatus = SpecFindingStatusDto;
export type ValidationIssue = SpecValidationIssueDto;
export type NewSpecRequest = NewSpecRequestDto;
export type SpecAction = SpecActionDto;
export type SpecDecision = SpecDecisionDto;
export type ImportCandidate = ImportCandidateDto;
export type SpecProgress = SpecProgressDto;

/** The documents in the order they are written, then the review that closes the spec. */
export const STAGES = ['proposal', 'specs', 'design', 'tasks', 'final'] as const;
export type Stage = (typeof STAGES)[number];

export function isStage(value: string): value is Stage {
  return (STAGES as readonly string[]).includes(value);
}
