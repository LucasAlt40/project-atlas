import type { ConditionDto, EndOutcomeDto, PositionDto } from '@/lib/tauri/commands';
import type { AgentNodePatch, Workflow, WorkflowEdge, WorkflowNode } from '../types';
import { resolvedPositions } from './graph';
import { ROW_GAP } from './layout';

/**
 * Edits of a workflow definition, as pure functions: each returns a new workflow (or the same
 * one when the edit makes no sense). The rules of what a valid workflow is stay in the core,
 * which validates; these only keep the graph coherent (no edge to a node that is gone).
 */

export function uniqueId(prefix: string, taken: Iterable<string>): string {
  const used = new Set(taken);
  if (!used.has(prefix)) return prefix;
  let n = 2;
  while (used.has(`${prefix}-${String(n)}`)) n += 1;
  return `${prefix}-${String(n)}`;
}

function slug(text: string): string {
  const base = text
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');
  return base || 'node';
}

/** Below everything that is already on the canvas. */
function nextPosition(workflow: Workflow): PositionDto {
  const positions = [...resolvedPositions(workflow).values()];
  if (positions.length === 0) return { x: 0, y: 0 };
  const lowest = Math.max(...positions.map((p) => p.y));
  const left = Math.min(...positions.map((p) => p.x));
  return { x: left, y: lowest + ROW_GAP };
}

function withNode(workflow: Workflow, node: WorkflowNode): Workflow {
  return { ...workflow, nodes: [...workflow.nodes, node] };
}

export function addAgentNode(workflow: Workflow, agentId: string, label: string): Workflow {
  const id = uniqueId(
    slug(label),
    workflow.nodes.map((n) => n.id),
  );
  return withNode(workflow, {
    id,
    type: 'agent',
    agentId,
    label,
    instructions: '',
    retryPolicy: { maxRetries: 0 },
    failurePolicy: { type: 'stop_workflow' },
    executionPolicy: { isolation: 'from_agent' },
    priority: 0,
    loopPolicy: null,
    position: nextPosition(workflow),
  });
}

export function addConditionNode(workflow: Workflow, label: string): Workflow {
  const id = uniqueId(
    slug(label),
    workflow.nodes.map((n) => n.id),
  );
  return withNode(workflow, {
    id,
    type: 'condition',
    label,
    // The verdict a step declared (`pass`, `approved`…), not whether the step ran.
    condition: { field: 'result.outcome', operator: 'equals', value: 'pass' },
    priority: 0,
    loopPolicy: null,
    position: nextPosition(workflow),
  });
}

export function addEndNode(
  workflow: Workflow,
  label: string,
  outcome: EndOutcomeDto = 'done',
): Workflow {
  const id = uniqueId(
    slug(label),
    workflow.nodes.map((n) => n.id),
  );
  return withNode(workflow, {
    id,
    type: 'end',
    label,
    outcome,
    priority: 0,
    loopPolicy: null,
    position: nextPosition(workflow),
  });
}

/** Removes the node and everything that pointed at it: its edges and failure routes. */
export function removeNode(workflow: Workflow, nodeId: string): Workflow {
  return {
    ...workflow,
    nodes: workflow.nodes
      .filter((n) => n.id !== nodeId)
      .map((n) =>
        n.type === 'agent' &&
        n.failurePolicy.type === 'route_to_node' &&
        n.failurePolicy.nodeId === nodeId
          ? { ...n, failurePolicy: { type: 'stop_workflow' as const } }
          : n,
      ),
    edges: workflow.edges.filter((e) => e.sourceNodeId !== nodeId && e.targetNodeId !== nodeId),
  };
}

function sameCondition(a: ConditionDto | null, b: ConditionDto | null): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

export function connect(
  workflow: Workflow,
  sourceId: string,
  targetId: string,
  condition: ConditionDto | null = null,
  label = '',
): Workflow {
  const source = workflow.nodes.find((n) => n.id === sourceId);
  const target = workflow.nodes.find((n) => n.id === targetId);
  if (!source || !target || source.type === 'end') return workflow;
  if (
    workflow.edges.some(
      (e) =>
        e.sourceNodeId === sourceId &&
        e.targetNodeId === targetId &&
        sameCondition(e.condition, condition),
    )
  ) {
    return workflow;
  }
  const edge: WorkflowEdge = {
    id: uniqueId(
      `${sourceId}->${targetId}`,
      workflow.edges.map((e) => e.id),
    ),
    sourceNodeId: sourceId,
    targetNodeId: targetId,
    condition,
    label,
  };
  return { ...workflow, edges: [...workflow.edges, edge] };
}

export function disconnect(workflow: Workflow, edgeId: string): Workflow {
  return { ...workflow, edges: workflow.edges.filter((e) => e.id !== edgeId) };
}

export function updateEdge(
  workflow: Workflow,
  edgeId: string,
  patch: Partial<Pick<WorkflowEdge, 'condition' | 'label'>>,
): Workflow {
  return {
    ...workflow,
    edges: workflow.edges.map((e) => (e.id === edgeId ? { ...e, ...patch } : e)),
  };
}

export function updateAgentNode(
  workflow: Workflow,
  nodeId: string,
  patch: AgentNodePatch,
): Workflow {
  return {
    ...workflow,
    nodes: workflow.nodes.map((n) =>
      n.id === nodeId && n.type === 'agent' ? { ...n, ...patch } : n,
    ),
  };
}

export function updateNodeLabel(workflow: Workflow, nodeId: string, label: string): Workflow {
  return {
    ...workflow,
    nodes: workflow.nodes.map((n) => (n.id === nodeId ? { ...n, label } : n)),
  };
}

export function updateCondition(
  workflow: Workflow,
  nodeId: string,
  condition: ConditionDto,
): Workflow {
  return {
    ...workflow,
    nodes: workflow.nodes.map((n) =>
      n.id === nodeId && n.type === 'condition' ? { ...n, condition } : n,
    ),
  };
}

export function updateEndOutcome(
  workflow: Workflow,
  nodeId: string,
  outcome: EndOutcomeDto,
): Workflow {
  return {
    ...workflow,
    nodes: workflow.nodes.map((n) => (n.id === nodeId && n.type === 'end' ? { ...n, outcome } : n)),
  };
}

/** Where a failed step goes once its retries are used: another node, or nowhere (it stops). */
export function setFailureRoute(
  workflow: Workflow,
  nodeId: string,
  targetId: string | null,
): Workflow {
  return updateAgentNode(workflow, nodeId, {
    failurePolicy: targetId
      ? { type: 'route_to_node', nodeId: targetId }
      : { type: 'stop_workflow' },
  });
}

/** Bounds a loop through the node: it may start at most `max` times. `null` removes the bound. */
export function setLoopLimit(workflow: Workflow, nodeId: string, max: number | null): Workflow {
  const taken = workflow.nodes.flatMap((n) =>
    n.id !== nodeId && n.loopPolicy ? [n.loopPolicy.loopId] : [],
  );
  return {
    ...workflow,
    nodes: workflow.nodes.map((n) => {
      if (n.id !== nodeId) return n;
      if (max === null) return { ...n, loopPolicy: null };
      return {
        ...n,
        loopPolicy: {
          loopId: n.loopPolicy?.loopId ?? uniqueId(`loop-${n.id}`, taken),
          maxIterations: max,
        },
      };
    }),
  };
}

/** Forgets where nodes were put: the layout places them again. */
export function resetLayout(workflow: Workflow): Workflow {
  return { ...workflow, nodes: workflow.nodes.map((n) => ({ ...n, position: null })) };
}

/** The same process as a custom workflow. Only who made the graph changes. */
export function asCustom(workflow: Workflow): Workflow {
  return { ...workflow, mode: 'custom' };
}
