import type { Workspace } from '../types';

/**
 * This version shows at most this many agents in a workspace. A UI limit only: the core's
 * agents, workspaces and layouts have no such rule (the layout's grid size is data).
 */
export const MAX_WORKSPACE_AGENTS = 4;

export function canAddAgent(workspace: Workspace): boolean {
  return workspace.layout.agentPlacements.length < MAX_WORKSPACE_AGENTS;
}

export interface GridCell {
  row: number;
  column: number;
  /** The agent placed here, if any. */
  agentId: string | null;
}

/** Every cell of the grid, row by row, with the agent that occupies it. */
export function gridCells(workspace: Workspace): GridCell[] {
  const { rows, columns, agentPlacements } = workspace.layout;
  const cells: GridCell[] = [];
  for (let row = 0; row < rows; row += 1) {
    for (let column = 0; column < columns; column += 1) {
      const placement = agentPlacements.find(
        (p) => p.position.row === row && p.position.column === column,
      );
      cells.push({ row, column, agentId: placement?.agentId ?? null });
    }
  }
  return cells;
}
