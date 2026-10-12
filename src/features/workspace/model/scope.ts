import type { ScopeRequestDto, WorkspaceProjectDto } from '@/lib/tauri/commands';

/**
 * Which project an agent works in (the only one it may change, in its isolated worktree) and
 * which others it may only consult. Always complete: the screen never holds a half-chosen scope.
 */
export interface ScopeChoice {
  workingProjectId: string;
  readProjectIds: string[];
}

interface HasProjects {
  projects: WorkspaceProjectDto[];
}

/** The primary project alone: what an agent did before a workspace could have several. */
export function defaultChoice(workspace: HasProjects): ScopeChoice {
  return { workingProjectId: workspace.projects[0]?.id ?? '', readProjectIds: [] };
}

/**
 * A stored choice made valid for the workspace as it is now: a project that was removed is
 * dropped, a working project that is gone becomes the primary one, and the working project is
 * never also listed as consulted.
 */
export function normalizeChoice(
  choice: ScopeChoice | undefined,
  workspace: HasProjects,
): ScopeChoice {
  const ids = new Set(workspace.projects.map((p) => p.id));
  const fallback = defaultChoice(workspace);
  if (!choice) return fallback;
  const workingProjectId = ids.has(choice.workingProjectId)
    ? choice.workingProjectId
    : fallback.workingProjectId;
  const wanted = new Set(choice.readProjectIds);
  // In the workspace's order, whatever order the choice was made in.
  const readProjectIds = workspace.projects
    .filter((p) => p.id !== workingProjectId && wanted.has(p.id))
    .map((p) => p.id);
  return { workingProjectId, readProjectIds };
}

/** What is sent to the core: nothing for the default scope, so a one-project workspace sends what it always sent. */
export function toScopeRequest(
  choice: ScopeChoice | undefined,
  workspace: HasProjects,
): ScopeRequestDto | undefined {
  const normal = normalizeChoice(choice, workspace);
  const fallback = defaultChoice(workspace);
  if (normal.workingProjectId === fallback.workingProjectId && normal.readProjectIds.length === 0) {
    return undefined;
  }
  return { workingProjectId: normal.workingProjectId, readProjectIds: normal.readProjectIds };
}

export function withWorking(choice: ScopeChoice, projectId: string): ScopeChoice {
  return {
    workingProjectId: projectId,
    readProjectIds: choice.readProjectIds.filter((id) => id !== projectId),
  };
}

export function withRead(choice: ScopeChoice, projectId: string, read: boolean): ScopeChoice {
  if (projectId === choice.workingProjectId) return choice;
  const others = choice.readProjectIds.filter((id) => id !== projectId);
  return { ...choice, readProjectIds: read ? [...others, projectId] : others };
}

export function consultsAll(choice: ScopeChoice, workspace: HasProjects): ScopeChoice {
  return {
    ...choice,
    readProjectIds: workspace.projects
      .filter((p) => p.id !== choice.workingProjectId)
      .map((p) => p.id),
  };
}

const KEY = 'atlas.scope.';

/** The scopes the user chose in this workspace, by agent. A convenience: it renders without it. */
export function loadScopes(workspaceId: string): Record<string, ScopeChoice> {
  try {
    const raw = localStorage.getItem(KEY + workspaceId);
    const parsed: unknown = raw ? JSON.parse(raw) : {};
    if (typeof parsed !== 'object' || parsed === null) return {};
    const result: Record<string, ScopeChoice> = {};
    for (const [agentId, value] of Object.entries(parsed)) {
      const v = value as Partial<ScopeChoice> | null;
      if (v && typeof v.workingProjectId === 'string' && Array.isArray(v.readProjectIds)) {
        result[agentId] = {
          workingProjectId: v.workingProjectId,
          readProjectIds: v.readProjectIds.filter((id): id is string => typeof id === 'string'),
        };
      }
    }
    return result;
  } catch {
    return {};
  }
}

export function saveScopes(workspaceId: string, scopes: Record<string, ScopeChoice>): void {
  try {
    localStorage.setItem(KEY + workspaceId, JSON.stringify(scopes));
  } catch {
    // The choice only lasts for this session.
  }
}
