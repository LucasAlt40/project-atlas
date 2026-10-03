import { MAX_WORKSPACE_AGENTS, canAddAgent, gridCells } from './grid';
import type { Workspace } from '../types';

const workspace = (placements: Workspace['layout']['agentPlacements']): Workspace => ({
  id: 'default',
  name: 'W',
  projectPath: '/p',
  description: null,
  createdAt: 1,
  updatedAt: 1,
  layout: { rows: 2, columns: 2, agentPlacements: placements },
});

describe('grid', () => {
  it('lists every cell of the grid with its occupant', () => {
    const cells = gridCells(workspace([{ agentId: 'a', position: { row: 1, column: 1 } }]));

    expect(cells).toHaveLength(4);
    expect(cells.filter((c) => c.agentId !== null)).toEqual([{ row: 1, column: 1, agentId: 'a' }]);
    expect(cells[0]).toEqual({ row: 0, column: 0, agentId: null });
  });

  it('limits the UI to four agents without limiting the layout size from the core', () => {
    const placements = (n: number) =>
      Array.from({ length: n }, (_, i) => ({
        agentId: `a${String(i)}`,
        position: { row: Math.floor(i / 2), column: i % 2 },
      }));

    expect(MAX_WORKSPACE_AGENTS).toBe(4);
    expect(canAddAgent(workspace(placements(3)))).toBe(true);
    expect(canAddAgent(workspace(placements(4)))).toBe(false);
  });
});
