import { useCallback, useMemo, useState } from 'react';
import {
  Background,
  BackgroundVariant,
  MiniMap,
  ReactFlow,
  applyNodeChanges,
  type Connection,
  type NodeChange,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { useI18n } from '@/i18n/I18nProvider';
import type { PositionDto } from '@/lib/tauri/commands';
import {
  FAILURE_EDGE_PREFIX,
  positionsOf,
  toFlow,
  type AgentLabel,
  type FlowNode,
} from '../model/graph';
import type { Workflow, WorkflowRun } from '../types';
import { NODE_SIZE, NODE_TYPES } from './WorkflowNodes';
import styles from './Workflow.module.css';

export type Selection = { kind: 'node'; id: string } | { kind: 'edge'; id: string } | null;

interface Props {
  workflow: Workflow;
  run: WorkflowRun | undefined;
  agents: ReadonlyMap<string, AgentLabel>;
  invalidNodes: ReadonlySet<string>;
  /** The definition may be changed (no run holds on to it, and no run is being shown). */
  editable: boolean;
  selection: Selection;
  onSelect: (selection: Selection) => void;
  onMove: (positions: Map<string, PositionDto>) => void;
  onConnect: (source: string, target: string) => void;
  onDeleteNode: (id: string) => void;
  onDeleteEdge: (id: string) => void;
}

/**
 * The graph, drawn. The graph library is only a way to draw the workflow and to turn the user's
 * gestures into edits: the workflow stays the source of truth, and what the library knows about
 * (ids, positions, selection) is derived from it on every change.
 */
export function WorkflowCanvas({
  workflow,
  run,
  agents,
  invalidNodes,
  editable,
  selection,
  onSelect,
  onMove,
  onConnect,
  onDeleteNode,
  onDeleteEdge,
}: Props) {
  const { t } = useI18n();
  const flow = useMemo(
    () => toFlow(workflow, { run, agents, invalidNodes }),
    [workflow, run, agents, invalidNodes],
  );
  // What the library is told (and moves while a node is dragged) follows the workflow: when the
  // workflow changes, the nodes are derived from it again.
  const [nodes, setNodes] = useState<FlowNode[]>(flow.nodes);
  const [derivedFrom, setDerivedFrom] = useState(flow.nodes);
  if (derivedFrom !== flow.nodes) {
    setDerivedFrom(flow.nodes);
    setNodes(flow.nodes);
  }

  const shown = useMemo(
    () =>
      nodes.map((node) => ({
        ...node,
        initialWidth: NODE_SIZE.width,
        initialHeight: NODE_SIZE.height,
        selected: selection?.kind === 'node' && selection.id === node.id,
      })),
    [nodes, selection],
  );
  const edges = useMemo(
    () =>
      flow.edges.map((edge) => ({
        ...edge,
        selected: selection?.kind === 'edge' && selection.id === edge.id,
        animated: run?.nodes[edge.source]?.status === 'running',
      })),
    [flow.edges, selection, run],
  );

  const onNodesChange = useCallback((changes: NodeChange<FlowNode>[]) => {
    setNodes((current) => applyNodeChanges(changes, current));
  }, []);

  const onConnectNodes = useCallback(
    (connection: Connection) => {
      if (editable && connection.source && connection.target) {
        onConnect(connection.source, connection.target);
      }
    },
    [editable, onConnect],
  );

  return (
    <div className={styles.canvas} role="group" aria-label={t('workflow.canvas')}>
      <ReactFlow
        key={`${workflow.id}:${run?.id ?? 'editor'}`}
        nodes={shown}
        edges={edges}
        nodeTypes={NODE_TYPES}
        onNodesChange={onNodesChange}
        onNodeClick={(_, node) => {
          onSelect({ kind: 'node', id: node.id });
        }}
        onEdgeClick={(_, edge) => {
          // A failure route is a setting of its node while editing; in a run it carries handoffs.
          if (!editable && !run) return;
          if (editable && edge.id.startsWith(FAILURE_EDGE_PREFIX)) return;
          onSelect({ kind: 'edge', id: edge.id });
        }}
        onPaneClick={() => {
          onSelect(null);
        }}
        onNodeDragStop={() => {
          if (editable) onMove(positionsOf(nodes));
        }}
        onConnect={onConnectNodes}
        onNodesDelete={(deleted) => {
          if (editable) for (const node of deleted) onDeleteNode(node.id);
        }}
        onEdgesDelete={(deleted) => {
          if (editable) for (const edge of deleted) onDeleteEdge(edge.id);
        }}
        deleteKeyCode={editable ? ['Backspace', 'Delete'] : null}
        nodesDraggable={editable}
        nodesConnectable={editable}
        elementsSelectable
        fitView
        fitViewOptions={{ padding: 0.2, maxZoom: 1.1 }}
        minZoom={0.2}
        maxZoom={2}
        colorMode="dark"
      >
        <Background variant={BackgroundVariant.Dots} gap={22} size={1.5} color="#2d3748" />
        <MiniMap pannable zoomable ariaLabel={t('workflow.minimap')} />
      </ReactFlow>
    </div>
  );
}
