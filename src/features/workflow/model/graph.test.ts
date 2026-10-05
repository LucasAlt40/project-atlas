import {
  agentNode,
  edge,
  endNode,
  nodeState,
  passwordRecovery,
  run,
} from '@/test/workflowFixtures';
import { setFailureRoute } from './edit';
import {
  FAILURE_EDGE_PREFIX,
  conditionText,
  positionsOf,
  resolvedPositions,
  toFlow,
  withPositions,
} from './graph';
import { layeredPositions } from './layout';
import { NODE_STATUS_VIEW, nodeLabel, runProgress } from './status';
import { invalidNodeIds, issueMessage } from './validation';
import { createTranslator } from '@/i18n';

const agents = new Map([['a-qa', { name: 'QA Agent', personality: 'QA' }]]);

describe('domain workflow <-> graph view model', () => {
  it('draws every node and edge of the workflow with its own ids', () => {
    const w = passwordRecovery();
    const { nodes, edges } = toFlow(w, { agents });
    expect(nodes.map((n) => [n.id, n.type])).toEqual([
      ['architect', 'agent'],
      ['developer', 'agent'],
      ['qa', 'agent'],
      ['bug-fixer', 'agent'],
      ['done', 'end'],
    ]);
    expect(edges.map((e) => [e.id, e.source, e.target])).toEqual(
      w.edges.map((e) => [e.id, e.sourceNodeId, e.targetNodeId]),
    );
    // The agent's name comes from the catalog, never from the node.
    expect(nodes.find((n) => n.id === 'qa')?.data.agent).toEqual({
      name: 'QA Agent',
      personality: 'QA',
    });
    expect(nodes.find((n) => n.id === 'architect')?.data.agent).toBeNull();
  });

  it('labels conditional edges and draws a failure route as its own dashed edge', () => {
    const w = setFailureRoute(passwordRecovery(), 'developer', 'bug-fixer');
    const { edges } = toFlow(w, { agents });
    expect(edges.find((e) => e.id === 'qa->done:pass')?.label).toBe('pass');
    expect(edges.find((e) => e.id === 'developer->qa')?.label).toBeUndefined();
    const failure = edges.find((e) => e.id === `${FAILURE_EDGE_PREFIX}developer`);
    expect(failure).toMatchObject({ source: 'developer', target: 'bug-fixer' });
    expect(failure?.data?.failureRoute).toBe(true);
    expect(conditionText({ field: 'result.status', operator: 'equals', value: 'fail' })).toBe(
      'result.status = fail',
    );
    expect(conditionText({ field: 'result.summary', operator: 'not_exists' })).toBe(
      'result.summary not exists',
    );
  });

  it('carries the run status, the latest attempt and the loop of each node', () => {
    const w = passwordRecovery();
    const r = run(w, 'running', {
      architect: nodeState('completed', {
        attempts: [
          {
            attempt: 1,
            iteration: 1,
            executionId: 'exec-4',
            status: 'completed',
            startedAt: 1,
            completedAt: 2,
            summary: null,
            outcome: null,
            failure: null,
          },
        ],
      }),
      qa: nodeState('running', {
        iterations: 2,
        attempts: [
          {
            attempt: 2,
            iteration: 2,
            executionId: 'exec-9',
            status: 'running',
            startedAt: 1,
            completedAt: null,
            summary: null,
            outcome: null,
            failure: null,
          },
        ],
      }),
    });
    const { nodes } = toFlow(w, { agents, run: r });
    expect(nodes.find((n) => n.id === 'architect')?.data).toMatchObject({
      status: 'completed',
      attempt: { number: 1, executionId: 'exec-4' },
    });
    expect(nodes.find((n) => n.id === 'qa')?.data).toMatchObject({
      status: 'running',
      loop: { id: 'loop-qa', iteration: 2, max: 3 },
    });
    // Nodes the run has not reached are pending; with no run there is no status at all.
    expect(nodes.find((n) => n.id === 'bug-fixer')?.data.status).toBe('pending');
    expect(toFlow(w, { agents }).nodes[0]?.data.status).toBeNull();
  });

  it('marks the nodes validation complained about', () => {
    const w = passwordRecovery();
    const { nodes } = toFlow(w, { agents, invalidNodes: new Set(['developer']) });
    expect(nodes.find((n) => n.id === 'developer')?.data.invalid).toBe(true);
    expect(nodes.find((n) => n.id === 'qa')?.data.invalid).toBe(false);
  });

  it('keeps where the user put a node and lays out the rest', () => {
    const w = passwordRecovery();
    const placed = {
      ...w,
      nodes: w.nodes.map((n) => (n.id === 'qa' ? { ...n, position: { x: 400, y: 9 } } : n)),
    };
    const positions = resolvedPositions(placed);
    expect(positions.get('qa')).toEqual({ x: 400, y: 9 });
    expect(positions.get('architect')?.y).toBeLessThan(positions.get('developer')?.y ?? 0);
  });

  it('turns canvas positions back into plain coordinates on the workflow', () => {
    const w = passwordRecovery();
    const flow = toFlow(w, { agents });
    const moved = flow.nodes.map((n) => (n.id === 'done' ? { ...n, position: { x: 1, y: 2 } } : n));
    const saved = withPositions(w, positionsOf(moved));
    expect(saved.nodes.find((n) => n.id === 'done')?.position).toEqual({ x: 1, y: 2 });
    // What is saved holds no trace of the library: no `measured`, no `selected`, no handles.
    expect(Object.keys(saved.nodes[0] ?? {}).sort()).toEqual(Object.keys(w.nodes[0] ?? {}).sort());
    expect(JSON.stringify(saved)).not.toMatch(/measured|handleBounds|selected/);
  });
});

describe('layout', () => {
  it('layers from the start downwards and draws a loop going back up', () => {
    const w = passwordRecovery();
    const positions = layeredPositions(
      w.nodes.map((n) => n.id),
      w.edges.map((e) => ({ source: e.sourceNodeId, target: e.targetNodeId })),
    );
    const y = (id: string) => positions.get(id)?.y ?? -1;
    expect(y('architect')).toBe(0);
    expect(y('developer')).toBeGreaterThan(y('architect'));
    expect(y('qa')).toBeGreaterThan(y('developer'));
    // The way back (bug fixer -> qa) does not push QA below the bug fixer.
    expect(y('bug-fixer')).toBeGreaterThan(y('qa'));
    expect(y('done')).toBeGreaterThan(y('qa'));
  });

  it('puts independent branches side by side and a join below both', () => {
    const w = {
      ...passwordRecovery(),
      nodes: [
        agentNode('a', 'x', 'A'),
        agentNode('b', 'x', 'B'),
        agentNode('c', 'x', 'C'),
        agentNode('d', 'x', 'D'),
        endNode('e'),
      ],
      edges: [edge('a', 'b'), edge('a', 'c'), edge('b', 'd'), edge('c', 'd'), edge('d', 'e')],
    };
    const p = layeredPositions(
      w.nodes.map((n) => n.id),
      w.edges.map((e) => ({ source: e.sourceNodeId, target: e.targetNodeId })),
    );
    expect(p.get('b')?.y).toBe(p.get('c')?.y);
    expect(p.get('b')?.x).not.toBe(p.get('c')?.x);
    expect(p.get('d')?.y).toBeGreaterThan(p.get('b')?.y ?? 0);
  });

  it('copes with a graph that is all cycle and with links to nowhere', () => {
    const p = layeredPositions(
      ['a', 'b'],
      [
        { source: 'a', target: 'b' },
        { source: 'b', target: 'a' },
        { source: 'a', target: 'ghost' },
      ],
    );
    expect(p.size).toBe(2);
  });
});

describe('status and progress', () => {
  it('counts what is done without counting the end nodes, and says it in symbols and words', () => {
    const w = passwordRecovery();
    const r = run(w, 'running', {
      architect: nodeState('completed'),
      developer: nodeState('running'),
      qa: nodeState('failed'),
      'bug-fixer': nodeState('blocked'),
    });
    expect(runProgress(r)).toEqual({
      total: 4,
      completed: 1,
      running: 1,
      waiting: 0,
      pending: 0,
      failed: 1,
      blocked: 1,
    });
    expect(nodeLabel(r, 'qa')).toBe('QA');
    expect(nodeLabel(r, 'ghost')).toBe('ghost');
    // Every state has a symbol of its own, so colour is never the only signal.
    const symbols = Object.values(NODE_STATUS_VIEW).map((v) => v.symbol);
    expect(new Set(symbols).size).toBe(symbols.length);
  });
});

describe('validation messages', () => {
  const t = createTranslator('en-US');

  it('words an issue naming the node, in both languages', () => {
    const w = passwordRecovery();
    const issue = {
      code: 'missing_agent',
      severity: 'error',
      nodeId: 'developer',
      edgeId: null,
      params: {},
    } as const;
    expect(issueMessage(t, issue, w)).toBe('Developer has no agent yet: choose one.');
    expect(issueMessage(createTranslator('pt-BR'), issue, w)).toBe(
      'Developer ainda não tem agente: escolha um.',
    );
    const cycle = {
      code: 'cycle_without_limit',
      severity: 'error',
      nodeId: 'qa',
      edgeId: null,
      params: { nodes: 'qa,bug-fixer' },
    } as const;
    expect(issueMessage(t, cycle, w)).toContain('QA, Bug Fixer');
    const edgeIssue = {
      code: 'end_has_outgoing',
      severity: 'error',
      nodeId: null,
      edgeId: 'qa->done:pass',
      params: {},
    } as const;
    expect(issueMessage(t, edgeIssue, w)).toContain('QA → Done');
  });

  it('finds the nodes issues are about, including through edges and cycles', () => {
    const w = passwordRecovery();
    const ids = invalidNodeIds(
      [
        { code: 'missing_agent', severity: 'error', nodeId: 'developer', edgeId: null, params: {} },
        {
          code: 'end_has_outgoing',
          severity: 'error',
          nodeId: null,
          edgeId: 'qa->done:pass',
          params: {},
        },
        {
          code: 'cycle_without_limit',
          severity: 'error',
          nodeId: null,
          edgeId: null,
          params: { nodes: 'bug-fixer' },
        },
      ],
      w,
    );
    expect([...ids].sort()).toEqual(['bug-fixer', 'developer', 'done', 'qa']);
  });
});
