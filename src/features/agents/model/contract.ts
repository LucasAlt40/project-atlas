import type { Translate } from '@/i18n';
import type { ContractKindDto, OutcomeDto, ResultContractDto } from '@/lib/tauri/commands';

export type ResultContract = ResultContractDto;
export type Outcome = OutcomeDto;
export type ContractKind = ContractKindDto;

export const CONTRACT_KINDS: readonly ContractKind[] = [
  'general',
  'validation',
  'review',
  'implementation',
  'custom',
];

/** The ids a preset offers. The ids are stable (workflow edges refer to them): they are never
 * translated; only the labels and descriptions are. */
const PRESET_IDS: Record<Exclude<ContractKind, 'general' | 'custom'>, readonly string[]> = {
  validation: ['pass', 'fail'],
  review: ['approved', 'changes_requested'],
  implementation: ['implemented', 'partial', 'blocked'],
};

export function presetOutcomes(kind: ContractKind, t: Translate): Outcome[] {
  if (kind === 'general' || kind === 'custom') return [];
  return PRESET_IDS[kind].map((id) => ({
    id,
    label: t(`contract.outcome.${id}` as Parameters<Translate>[0]),
    description: t(`contract.outcome.${id}.description` as Parameters<Translate>[0]),
  }));
}

export function presetContract(kind: ContractKind, t: Translate): ResultContract {
  return { kind, outcomes: presetOutcomes(kind, t) };
}

export const GENERAL_CONTRACT: ResultContract = { kind: 'general', outcomes: [] };

const ID_PATTERN = /^[a-z0-9_-]{1,40}$/;

export type ContractProblem = 'noOutcomes' | 'invalidId' | 'duplicateId' | 'emptyLabel';

/** What is wrong with a contract before it is sent (the core checks again). */
export function contractProblem(contract: ResultContract): ContractProblem | null {
  if (contract.kind === 'general') return null;
  if (contract.outcomes.length === 0) return 'noOutcomes';
  const seen = new Set<string>();
  for (const outcome of contract.outcomes) {
    const id = outcome.id.trim();
    if (!ID_PATTERN.test(id)) return 'invalidId';
    if (seen.has(id)) return 'duplicateId';
    seen.add(id);
    if (outcome.label.trim() === '') return 'emptyLabel';
  }
  return null;
}

/** The label to show for an outcome id of a contract, falling back to the id itself. */
export function outcomeLabel(outcomes: readonly Outcome[], id: string): string {
  return outcomes.find((o) => o.id === id)?.label ?? id;
}
