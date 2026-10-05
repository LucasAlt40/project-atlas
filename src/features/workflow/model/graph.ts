import type { Edge, Node } from '@xyflow/react';
import type { ConditionDto, NodeStatusDto, PositionDto } from '@/lib/tauri/commands';
import type { Workflow, WorkflowNode, WorkflowRun } from '../types';
import { layeredPositions } from './layout';

/**
 * The only place the workflow is translated into what the graph library draws, and back. The
 * domain model keeps its own ids and plain coordinates; nothing here is persisted.
 */

/** The prefix of the id of the dashed edge that draws a node's failure route. */
export const FAILURE_EDGE_PREFIX = 'failure:';

export interface AgentLabel {
  name: string;
  personality: string;
}

export interface FlowNodeData extends Record<string, unknown> {
  node: WorkflowNode;
  /** The agent's name and personality, from the catalog (never stored on the node). */
  agent: AgentLabel | null;
  /** Where the node is in the run being shown; `null` when no run is shown. */
  status: NodeStatusDto | null;
  /** The latest attempt: its number and the execution it is. */
  attempt: { number: number; iteration: number; executionId: string } | null;
  /** The loop the node bounds, with how far it has gone. */
  loop: { id: string; iteration: number; max: number } | null;
  /** Something is wrong with this node (validation). */
  invalid: boolean;
}

export type FlowNode = Node<FlowNodeData, 'agent' | 'condition' | 'end'>;

export interface FlowEdgeData extends Record<string, unknown> {
  failureRoute: boolean;
}
export type FlowEdge = Edge<FlowEdgeData>;

const EDGE_BAD = '#ffb4ab';
const EDGE_GOOD = '#7bd0ff';

/** A road that says it is the bad one (fail, changes requested) is drawn red and dashed, the good one blue. */
function routeLook(text: string): Pick<FlowEdge, 'style' | 'labelStyle'> {
  if (/\b(fail|failed|changes_requested|blocked|rejected)\b/i.test(text)) {
    return { style: { stroke: EDGE_BAD, strokeDasharray: '5 4' }, labelStyle: { fill: EDGE_BAD } };
  }
  if (/\b(pass|passed|approved|implemented|success|done)\b/i.test(text)) {
    return { style: { stroke: EDGE_GOOD }, labelStyle: { fill: EDGE_GOOD } };
  }
  return {};
}

/** `result.status = fail`: how a condition reads on an edge that has no label of its own. */
export function conditionText(condition: ConditionDto): string {
  const operator = {
    equals: '=',
    not_equals: '≠',
    exists: 'exists',
    not_exists: 'not exists',
  }[condition.operator];
  const value = condition.operator === 'equals' || condition.operator === 'not_equals';
  return value
    ? `${condition.field} ${operator} ${condition.value ?? ''}`
    : `${condition.field} ${operator}`;
}

/** Where each node sits: where the user put it, or where the layout would. */
export function resolvedPositions(workflow: Workflow): Map<string, PositionDto> {
  const links = [
    ...workflow.edges.map((e) => ({ source: e.sourceNodeId, target: e.targetNodeId })),
    ...workflow.nodes.flatMap((n) =>
      n.type === 'agent' && n.failurePolicy.type === 'route_to_node'
        ? [{ source: n.id, target: n.failurePolicy.nodeId }]
        : [],
    ),
  ];
  const layered = layeredPositions(
    workflow.nodes.map((n) => n.id),
    links,
  );
  return new Map(
    workflow.nodes.map((n) => [n.id, n.position ?? layered.get(n.id) ?? { x: 0, y: 0 }]),
  );
}

export interface FlowOptions {
  run?: WorkflowRun | undefined;
  agents: ReadonlyMap<string, AgentLabel>;
  /** Ids of the nodes validation has something to say about. */
  invalidNodes?: ReadonlySet<string>;
}

export function toFlow(
  workflow: Workflow,
  options: FlowOptions,
): { nodes: FlowNode[]; edges: FlowEdge[] } {
  const positions = resolvedPositions(workflow);
  const nodes: FlowNode[] = workflow.nodes.map((node) => {
    const state = options.run?.nodes[node.id];
    const last = state?.attempts[state.attempts.length - 1];
    return {
      id: node.id,
      type: node.type,
      position: positions.get(node.id) ?? { x: 0, y: 0 },
      data: {
        node,
        agent: node.type === 'agent' ? (options.agents.get(node.agentId) ?? null) : null,
        status: options.run ? (state?.status ?? 'pending') : null,
        attempt: last
          ? { number: last.attempt, iteration: last.iteration, executionId: last.executionId }
          : null,
        loop:
          node.loopPolicy && state
            ? {
                id: node.loopPolicy.loopId,
                iteration: state.iterations,
                max: node.loopPolicy.maxIterations,
              }
            : node.loopPolicy
              ? { id: node.loopPolicy.loopId, iteration: 0, max: node.loopPolicy.maxIterations }
              : null,
        invalid: options.invalidNodes?.has(node.id) ?? false,
      },
    };
  });
  const edges: FlowEdge[] = workflow.edges.map((edge) => {
    const label = edge.label || (edge.condition ? conditionText(edge.condition) : undefined);
    return {
      id: edge.id,
      source: edge.sourceNodeId,
      target: edge.targetNodeId,
      label,
      data: { failureRoute: false },
      ...routeLook(`${label ?? ''} ${edge.condition?.value ?? ''}`),
    };
  });
  for (const node of workflow.nodes) {
    if (node.type === 'agent' && node.failurePolicy.type === 'route_to_node') {
      edges.push({
        id: `${FAILURE_EDGE_PREFIX}${node.id}`,
        source: node.id,
        target: node.failurePolicy.nodeId,
        label: 'failure',
        data: { failureRoute: true },
        style: { strokeDasharray: '6 4', stroke: EDGE_BAD },
        labelStyle: { fill: EDGE_BAD },
        deletable: false,
      });
    }
  }
  return { nodes, edges };
}

/** The workflow with each node where the canvas has it. Nothing else changes. */
export function withPositions(
  workflow: Workflow,
  positions: ReadonlyMap<string, PositionDto>,
): Workflow {
  return {
    ...workflow,
    nodes: workflow.nodes.map((node) => {
      const at = positions.get(node.id);
      return at ? { ...node, position: { x: at.x, y: at.y } } : node;
    }),
  };
}

export function positionsOf(nodes: readonly FlowNode[]): Map<string, PositionDto> {
  return new Map(nodes.map((n) => [n.id, { x: n.position.x, y: n.position.y }]));
}
