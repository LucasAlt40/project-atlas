import type { PositionDto } from '@/lib/tauri/commands';

const COLUMN_GAP = 290;
const ROW_GAP = 170;

interface Link {
  source: string;
  target: string;
}

/**
 * Where to draw each node when the user has not placed it: layers from the start nodes down,
 * left to right inside a layer. A link that closes a loop is not counted when layering, so a
 * `QA -> Bug Fixer -> QA` loop reads top to bottom with its way back drawn upwards. Only
 * presentation: the result is never read by the engine.
 */
export function layeredPositions(
  nodeIds: readonly string[],
  links: readonly Link[],
): Map<string, PositionDto> {
  const known = new Set(nodeIds);
  const valid = links.filter((l) => known.has(l.source) && known.has(l.target));
  const outgoing = new Map<string, string[]>();
  const incoming = new Map<string, number>();
  for (const id of nodeIds) {
    outgoing.set(id, []);
    incoming.set(id, 0);
  }
  for (const { source, target } of valid) {
    outgoing.get(source)?.push(target);
    incoming.set(target, (incoming.get(target) ?? 0) + 1);
  }

  // Depth-first from the nodes nothing leads to; an edge into a node still being visited is
  // the edge that closes a loop.
  const state = new Map<string, 'visiting' | 'done'>();
  const back = new Set<string>();
  const visit = (id: string) => {
    state.set(id, 'visiting');
    for (const next of outgoing.get(id) ?? []) {
      const seen = state.get(next);
      if (seen === 'visiting') back.add(`${id}>${next}`);
      else if (seen === undefined) visit(next);
    }
    state.set(id, 'done');
  };
  for (const id of nodeIds) if ((incoming.get(id) ?? 0) === 0 && !state.has(id)) visit(id);
  for (const id of nodeIds) if (!state.has(id)) visit(id);

  const forward = valid.filter((l) => !back.has(`${l.source}>${l.target}`));
  const layer = new Map<string, number>(nodeIds.map((id) => [id, 0]));
  // Longest path over the acyclic remainder, relaxed until nothing moves (it cannot take more
  // passes than there are nodes).
  let moved = true;
  let passes = nodeIds.length;
  while (moved && passes > 0) {
    moved = false;
    passes -= 1;
    for (const { source, target } of forward) {
      const wanted = (layer.get(source) ?? 0) + 1;
      if (wanted > (layer.get(target) ?? 0)) {
        layer.set(target, wanted);
        moved = true;
      }
    }
  }

  const rows = new Map<number, string[]>();
  for (const id of nodeIds) {
    const row = layer.get(id) ?? 0;
    rows.set(row, [...(rows.get(row) ?? []), id]);
  }
  const positions = new Map<string, PositionDto>();
  for (const [row, ids] of rows) {
    ids.forEach((id, index) => {
      positions.set(id, { x: (index - (ids.length - 1) / 2) * COLUMN_GAP, y: row * ROW_GAP });
    });
  }
  return positions;
}

export { ROW_GAP };
