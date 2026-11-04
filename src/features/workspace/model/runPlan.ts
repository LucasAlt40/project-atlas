import type { ScopeRequestDto, WorkspaceProjectDto } from '@/lib/tauri/commands';

/**
 * Which projects a workflow run (or a spec's implementation) changes, in order, and which it only
 * consults (ADR 0043). Each project changed is a run of its own, one after the other, each with
 * its own worktree, review and merge; every run consults the other projects of the plan.
 */
export interface RunPlan {
  /** In the order the runs go. Never empty for a workspace with projects. */
  workingProjectIds: string[];
  /** Consulted only, never changed. Never one of `workingProjectIds`. */
  readProjectIds: string[];
  /** For several runs: the next project runs only once the previous one's code is applied or kept. */
  waitForDelivery?: boolean;
}

interface HasProjects {
  projects: WorkspaceProjectDto[];
}

/** Above this many projects changed Atlas warns and suggests changing one, consulting the rest. */
export const MANY_PROJECTS = 3;

export function hasManyProjects(workspace: HasProjects): boolean {
  return workspace.projects.length > MANY_PROJECTS;
}

/**
 * The projects in the order they are worked in: a project after the ones it depends on (so the
 * backend goes before the frontend that consumes it), otherwise in the workspace's order.
 */
export function orderByDependencies(projects: WorkspaceProjectDto[]): WorkspaceProjectDto[] {
  const byId = new Map(projects.map((p) => [p.id, p]));
  const done = new Set<string>();
  const visiting = new Set<string>();
  const out: WorkspaceProjectDto[] = [];
  const visit = (project: WorkspaceProjectDto) => {
    if (done.has(project.id) || visiting.has(project.id)) return;
    visiting.add(project.id);
    for (const id of project.dependsOn) {
      const dependency = byId.get(id);
      if (dependency) visit(dependency);
    }
    visiting.delete(project.id);
    done.add(project.id);
    out.push(project);
  };
  for (const project of projects) visit(project);
  return out;
}

/** Every project is changed, in dependency order. */
export function allWorking(workspace: HasProjects): RunPlan {
  return {
    workingProjectIds: orderByDependencies(workspace.projects).map((p) => p.id),
    readProjectIds: [],
  };
}

/** One project is changed (the primary one unless named) and every other is consulted. */
export function oneWorking(workspace: HasProjects, projectId?: string): RunPlan {
  const working =
    workspace.projects.find((p) => p.id === projectId)?.id ?? workspace.projects[0]?.id;
  if (!working) return { workingProjectIds: [], readProjectIds: [] };
  return {
    workingProjectIds: [working],
    readProjectIds: workspace.projects.filter((p) => p.id !== working).map((p) => p.id),
  };
}

/**
 * What a run does when nothing was chosen: only the primary project is changed and the others are
 * consulted. Another project is run only when the person marks it (ADR 0043), and even then only
 * if the run before it says it needs something.
 */
export function suggestedPlan(workspace: HasProjects): RunPlan {
  return oneWorking(workspace);
}

/**
 * A stored plan made valid for the workspace as it is now: removed projects are dropped, a plan
 * left with no project to change falls back to the suggestion, and a project is never both
 * changed and consulted.
 */
export function normalizePlan(plan: RunPlan | undefined, workspace: HasProjects): RunPlan {
  if (!plan) return suggestedPlan(workspace);
  const ids = new Set(workspace.projects.map((p) => p.id));
  const working = [...new Set(plan.workingProjectIds)].filter((id) => ids.has(id));
  if (working.length === 0) return suggestedPlan(workspace);
  const wanted = new Set(plan.readProjectIds);
  const readProjectIds = workspace.projects
    .filter((p) => !working.includes(p.id) && wanted.has(p.id))
    .map((p) => p.id);
  return {
    workingProjectIds: working,
    readProjectIds,
    ...(plan.waitForDelivery ? { waitForDelivery: true } : {}),
  };
}

export type ProjectUse = 'work' | 'read' | 'none';

export function usageOf(plan: RunPlan, projectId: string): ProjectUse {
  if (plan.workingProjectIds.includes(projectId)) return 'work';
  return plan.readProjectIds.includes(projectId) ? 'read' : 'none';
}

/**
 * The plan with a project's use changed. A project newly changed goes where its dependencies put
 * it; the last project changed cannot stop being changed (a run always changes one).
 */
export function withUse(
  plan: RunPlan,
  projectId: string,
  use: ProjectUse,
  workspace: HasProjects,
): RunPlan {
  const without = {
    workingProjectIds: plan.workingProjectIds.filter((id) => id !== projectId),
    readProjectIds: plan.readProjectIds.filter((id) => id !== projectId),
  };
  if (use !== 'work' && without.workingProjectIds.length === 0) return plan;
  if (use === 'read') {
    return { ...without, readProjectIds: [...without.readProjectIds, projectId] };
  }
  if (use === 'none') return without;
  if (plan.workingProjectIds.includes(projectId)) return plan;
  const chosen = new Set([...without.workingProjectIds, projectId]);
  // The ones already chosen keep the order the person gave them; the new one is placed after
  // what it depends on.
  const ordered = orderByDependencies(workspace.projects).map((p) => p.id);
  const position = ordered.indexOf(projectId);
  const after = without.workingProjectIds.findIndex((id) => ordered.indexOf(id) > position);
  const workingProjectIds = [...without.workingProjectIds];
  workingProjectIds.splice(after === -1 ? workingProjectIds.length : after, 0, projectId);
  return {
    workingProjectIds: workingProjectIds.filter((id) => chosen.has(id)),
    readProjectIds: without.readProjectIds,
  };
}

/** Moves a changed project one place earlier (`-1`) or later (`1`) in the order of the runs. */
export function moved(plan: RunPlan, projectId: string, by: -1 | 1): RunPlan {
  const ids = [...plan.workingProjectIds];
  const from = ids.indexOf(projectId);
  const to = from + by;
  if (from < 0 || to < 0 || to >= ids.length) return plan;
  ids.splice(to, 0, ...ids.splice(from, 1));
  return { ...plan, workingProjectIds: ids };
}

/** Moves a changed project to `index` in the order of the runs (where it was dropped). */
export function movedTo(plan: RunPlan, projectId: string, index: number): RunPlan {
  const from = plan.workingProjectIds.indexOf(projectId);
  if (from < 0) return plan;
  const ids = plan.workingProjectIds.filter((id) => id !== projectId);
  const to = Math.max(0, Math.min(index, ids.length));
  if (to === from) return plan;
  ids.splice(to, 0, projectId);
  return { ...plan, workingProjectIds: ids };
}

/** Whether the plan is several runs, one per project. */
export function isChain(plan: RunPlan): boolean {
  return plan.workingProjectIds.length > 1;
}

/**
 * What is sent to the core. A plan that is the primary project alone sends nothing, so a
 * one-project workspace sends what it always sent.
 */
export function toStartRequest(
  plan: RunPlan,
  workspace: HasProjects,
): { scope?: ScopeRequestDto; nextProjectIds?: string[]; waitForDelivery?: boolean } {
  const normal = normalizePlan(plan, workspace);
  const [first, ...next] = normal.workingProjectIds;
  const primary = workspace.projects[0]?.id;
  const plain = first === primary && next.length === 0 && normal.readProjectIds.length === 0;
  if (!first || plain) return {};
  return {
    scope: { workingProjectId: first, readProjectIds: normal.readProjectIds },
    ...(next.length > 0 ? { nextProjectIds: next } : {}),
    ...(next.length > 0 && normal.waitForDelivery ? { waitForDelivery: true } : {}),
  };
}

const KEY = 'atlas.runPlan.';

/** The plan the person last chose for runs in this workspace. A convenience: it renders without it. */
export function loadRunPlan(workspaceId: string): RunPlan | undefined {
  try {
    const raw = localStorage.getItem(KEY + workspaceId);
    if (!raw) return undefined;
    const value = JSON.parse(raw) as Partial<RunPlan> | null;
    if (!value || !Array.isArray(value.workingProjectIds) || !Array.isArray(value.readProjectIds)) {
      return undefined;
    }
    return {
      workingProjectIds: value.workingProjectIds.filter(
        (id): id is string => typeof id === 'string',
      ),
      readProjectIds: value.readProjectIds.filter((id): id is string => typeof id === 'string'),
      ...(value.waitForDelivery === true ? { waitForDelivery: true } : {}),
    };
  } catch {
    return undefined;
  }
}

export function saveRunPlan(workspaceId: string, plan: RunPlan): void {
  try {
    localStorage.setItem(KEY + workspaceId, JSON.stringify(plan));
  } catch {
    // The choice only lasts for this session.
  }
}
