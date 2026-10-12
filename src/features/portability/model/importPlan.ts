import type {
  ImportDecisionDto,
  ImportItemPreviewDto,
  ImportPreviewDto,
  ImportRequestDto,
} from '@/lib/tauri/commands';

/** What the person has chosen so far, on top of what the core proposed. */
export interface Choices {
  workspaceId: string | null;
  decisions: Record<string, ImportDecisionDto>;
  /** Another runtime and model for an agent, by its `ref`. */
  models: Record<string, { runtimeId: string; modelId: string }>;
  /** Which of the person's own configurations stands for what the file does not carry. */
  bindings: Record<string, string>;
  /** `ref`s of items whose replacement the person confirmed. */
  confirmedReplacements: string[];
  keepPermissions: boolean;
}

/** What keeps the import from starting, and about which item (by `ref`) when it is about one. */
export type Blocker =
  | { code: 'workspace_required' }
  | { code: 'nothing_to_do' }
  | {
      code: 'model_choice_required' | 'replace_unconfirmed' | 'dependency_skipped';
      ref: string;
    }
  | { code: 'external_unbound'; ref: string };

/** Starts from the core's proposal. A same-name configuration offered for a dependency is only
 * pre-selected: nothing is applied until the person imports. */
export function initialChoices(preview: ImportPreviewDto, workspaceId: string | null): Choices {
  return {
    workspaceId,
    decisions: Object.fromEntries(preview.items.map((item) => [item.ref, item.defaultDecision])),
    models: {},
    bindings: Object.fromEntries(
      preview.external.flatMap((e) => (e.suggested ? [[e.ref, e.suggested] as const] : [])),
    ),
    confirmedReplacements: [],
    keepPermissions: false,
  };
}

export function decisionOf(item: ImportItemPreviewDto, choices: Choices): ImportDecisionDto {
  return choices.decisions[item.ref] ?? item.defaultDecision;
}

/** The item is going to be written (as a new one, or over one that exists). */
function writes(decision: ImportDecisionDto): boolean {
  return decision === 'create' || decision === 'replace';
}

export function blockers(preview: ImportPreviewDto, choices: Choices): Blocker[] {
  const found: Blocker[] = [];
  const decisions = new Map(preview.items.map((item) => [item.ref, decisionOf(item, choices)]));
  const going = preview.items.filter((item) => writes(decisionOf(item, choices)));
  if (going.length === 0) found.push({ code: 'nothing_to_do' });
  if (preview.needsWorkspace && !choices.workspaceId && going.some((i) => i.kind === 'workflow')) {
    found.push({ code: 'workspace_required' });
  }
  for (const item of going) {
    if (item.model && !item.model.keepAllowed && !choices.models[item.ref]) {
      found.push({ code: 'model_choice_required', ref: item.ref });
    }
    if (
      decisionOf(item, choices) === 'replace' &&
      !choices.confirmedReplacements.includes(item.ref)
    ) {
      found.push({ code: 'replace_unconfirmed', ref: item.ref });
    }
    for (const dependency of item.dependsOn) {
      if (decisions.has(dependency)) {
        if (decisions.get(dependency) === 'skip') {
          found.push({ code: 'dependency_skipped', ref: item.ref });
        }
      } else if (!choices.bindings[dependency]) {
        found.push({ code: 'external_unbound', ref: dependency });
      }
    }
  }
  return found;
}

/** The record without `key`. */
export function without<T>(record: Record<string, T>, key: string): Record<string, T> {
  return Object.fromEntries(Object.entries(record).filter(([name]) => name !== key));
}

/** Each blocker once: an unbound dependency is reported by every item that needs it. */
export function uniqueBlockers(list: Blocker[]): Blocker[] {
  const seen = new Set<string>();
  return list.filter((blocker) => {
    const key = `${blocker.code}:${'ref' in blocker ? blocker.ref : ''}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export interface Summary {
  create: number;
  replace: number;
  reuse: number;
  skip: number;
}

export function summarize(preview: ImportPreviewDto, choices: Choices): Summary {
  const summary: Summary = { create: 0, replace: 0, reuse: 0, skip: 0 };
  for (const item of preview.items) {
    const decision = decisionOf(item, choices);
    if (decision === 'create') summary.create += 1;
    else if (decision === 'replace') summary.replace += 1;
    else if (decision === 'use_existing') summary.reuse += 1;
    else summary.skip += 1;
  }
  return summary;
}

/** What goes to the core. Only choices that matter are sent: a model for an agent that is not
 * going to be written, or a binding nobody uses, would be refused as a mistake. */
export function toRequest(preview: ImportPreviewDto, choices: Choices): ImportRequestDto {
  const going = new Set(
    preview.items.filter((item) => writes(decisionOf(item, choices))).map((i) => i.ref),
  );
  const external = new Set(preview.external.map((e) => e.ref));
  return {
    workspaceId: choices.workspaceId,
    decisions: Object.fromEntries(preview.items.map((i) => [i.ref, decisionOf(i, choices)])),
    models: Object.fromEntries(Object.entries(choices.models).filter(([ref]) => going.has(ref))),
    bindings: Object.fromEntries(
      Object.entries(choices.bindings).filter(([ref]) => external.has(ref)),
    ),
    keepPermissionProfiles: choices.keepPermissions,
  };
}

/** Whether any agent that is going to be written asks for the permission to edit files. */
export function asksToEdit(preview: ImportPreviewDto, choices: Choices): boolean {
  return preview.items.some(
    (item) =>
      writes(decisionOf(item, choices)) && item.issues.some((i) => i.code === 'grants_editing'),
  );
}
