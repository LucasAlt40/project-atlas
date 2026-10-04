import { agentNode, edge, endNode, passwordRecovery } from '@/test/workflowFixtures';
import {
  addAgentNode,
  addConditionNode,
  addEndNode,
  asCustom,
  connect,
  disconnect,
  removeNode,
  resetLayout,
  setFailureRoute,
  setLoopLimit,
  uniqueId,
  updateAgentNode,
  updateEdge,
} from './edit';
import type { Workflow } from '../types';

const base = (): Workflow => passwordRecovery();

describe('workflow edits', () => {
  it('makes unique ids', () => {
    expect(uniqueId('qa', ['qa', 'qa-2'])).toBe('qa-3');
    expect(uniqueId('qa', [])).toBe('qa');
  });

  it('adds an agent node that only names the agent, below what is already drawn', () => {
    const next = addAgentNode(base(), 'a-new', 'Backend Developer');
    const added = next.nodes[next.nodes.length - 1];
    expect(added).toMatchObject({
      id: 'backend-developer',
      type: 'agent',
      agentId: 'a-new',
      label: 'Backend Developer',
    });
    // Nothing of the agent is copied into the node.
    expect(Object.keys(added ?? {})).not.toContain('personalityId');
    expect(Object.keys(added ?? {})).not.toContain('runtimeId');
    expect(added?.position?.y).toBeGreaterThan(0);
    expect(base().nodes).toHaveLength(5);
  });

  it('gives a second node of the same name its own id', () => {
    const once = addAgentNode(base(), 'a-qa', 'QA');
    expect(once.nodes.map((n) => n.id)).toContain('qa-2');
  });

  it('adds condition and end nodes', () => {
    const next = addEndNode(addConditionNode(base(), 'Condition'), 'Failed', 'failed');
    expect(next.nodes.find((n) => n.id === 'condition')).toMatchObject({
      type: 'condition',
      condition: { field: 'result.outcome', operator: 'equals' },
    });
    expect(next.nodes.find((n) => n.id === 'failed')).toMatchObject({
      type: 'end',
      outcome: 'failed',
    });
  });

  it('removes a node with its edges and any failure route that pointed at it', () => {
    const routed = setFailureRoute(base(), 'developer', 'bug-fixer');
    const next = removeNode(routed, 'bug-fixer');
    expect(next.nodes.map((n) => n.id)).not.toContain('bug-fixer');
    expect(
      next.edges.every((e) => e.sourceNodeId !== 'bug-fixer' && e.targetNodeId !== 'bug-fixer'),
    ).toBe(true);
    const developer = next.nodes.find((n) => n.id === 'developer');
    expect(developer?.type === 'agent' && developer.failurePolicy).toEqual({
      type: 'stop_workflow',
    });
  });

  it('connects two nodes once, and never from an end node or to a node that is not there', () => {
    const w = base();
    const next = connect(w, 'architect', 'qa');
    expect(next.edges).toHaveLength(w.edges.length + 1);
    expect(connect(next, 'architect', 'qa')).toBe(next);
    expect(connect(w, 'done', 'qa')).toBe(w);
    expect(connect(w, 'architect', 'ghost')).toBe(w);
    const conditional = connect(w, 'architect', 'qa', {
      field: 'result.status',
      operator: 'equals',
      value: 'pass',
    });
    expect(conditional.edges.at(-1)?.condition?.value).toBe('pass');
  });

  it('disconnects and edits a connection', () => {
    const w = base();
    expect(disconnect(w, 'architect->developer').edges).toHaveLength(w.edges.length - 1);
    const edited = updateEdge(w, 'architect->developer', { label: 'then' });
    expect(edited.edges[0]?.label).toBe('then');
  });

  it('sets the failure route and the loop limit of a node', () => {
    const routed = setFailureRoute(base(), 'developer', 'bug-fixer');
    const developer = routed.nodes.find((n) => n.id === 'developer');
    expect(developer?.type === 'agent' && developer.failurePolicy).toEqual({
      type: 'route_to_node',
      nodeId: 'bug-fixer',
    });
    const looped = setLoopLimit(base(), 'developer', 2);
    expect(looped.nodes.find((n) => n.id === 'developer')?.loopPolicy).toEqual({
      loopId: 'loop-developer',
      maxIterations: 2,
    });
    expect(
      setLoopLimit(looped, 'developer', null).nodes.find((n) => n.id === 'developer')?.loopPolicy,
    ).toBeNull();
    // A loop name is never reused.
    const again = setLoopLimit(setLoopLimit(base(), 'qa', 3), 'architect', 2);
    const names = again.nodes.flatMap((n) => (n.loopPolicy ? [n.loopPolicy.loopId] : []));
    expect(new Set(names).size).toBe(names.length);
  });

  it('updates an agent node without touching the others', () => {
    const next = updateAgentNode(base(), 'architect', { instructions: 'Design only.' });
    expect(next.nodes.find((n) => n.id === 'architect')).toMatchObject({
      instructions: 'Design only.',
    });
    expect(next.nodes.find((n) => n.id === 'developer')).toMatchObject({
      instructions: 'Implement the endpoint.',
    });
  });

  it('resets the layout and turns an automatic workflow into a custom one: same model', () => {
    const placed = {
      ...base(),
      nodes: base().nodes.map((n) => ({ ...n, position: { x: 5, y: 5 } })),
    };
    expect(resetLayout(placed).nodes.every((n) => n.position === null)).toBe(true);
    const custom = asCustom(base());
    expect(custom.mode).toBe('custom');
    expect({ ...custom, mode: 'automatic' }).toEqual(base());
  });

  it('tolerates editing a workflow with nothing in it', () => {
    const empty: Workflow = { ...base(), nodes: [], edges: [] };
    const next = addEndNode(empty, 'Done');
    expect(next.nodes).toEqual([expect.objectContaining({ id: 'done', position: { x: 0, y: 0 } })]);
    expect(agentNode('x', 'a', 'X').type).toBe('agent');
    expect(endNode('e').type).toBe('end');
    expect(edge('a', 'b').id).toBe('a->b');
  });
});
