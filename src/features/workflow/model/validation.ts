import type { Translate, TranslationKey } from '@/i18n';
import type { ValidationIssue, Workflow } from '../types';

/** Words one validation issue in the user's language, naming the node or edge it is about. */
export function issueMessage(t: Translate, issue: ValidationIssue, workflow: Workflow): string {
  const node = workflow.nodes.find((n) => n.id === issue.nodeId)?.label ?? issue.nodeId ?? '';
  const edge = workflow.edges.find((e) => e.id === issue.edgeId);
  const edgeName = edge
    ? `${workflow.nodes.find((n) => n.id === edge.sourceNodeId)?.label ?? edge.sourceNodeId} → ${
        workflow.nodes.find((n) => n.id === edge.targetNodeId)?.label ?? edge.targetNodeId
      }`
    : (issue.edgeId ?? '');
  const nodes = (issue.params.nodes ?? '')
    .split(',')
    .filter(Boolean)
    .map((id) => workflow.nodes.find((n) => n.id === id)?.label ?? id)
    .join(', ');
  // An agent whose outcomes have no route at all is worded apart from one missing a single route.
  const key =
    issue.code === 'outcome_without_route' && issue.params.outcomes
      ? 'workflow.issue.outcome_without_route_none'
      : `workflow.issue.${issue.code}`;
  return t(key as TranslationKey, {
    node,
    edge: edgeName,
    nodes,
    max: issue.params.max ?? '',
    target: issue.params.target ?? '',
    reason: issue.params.reason ?? '',
    outcome: issue.params.outcome ?? '',
    agent: issue.params.agent ?? '',
    status: issue.params.status ?? '',
    outcomes: issue.params.outcomes ?? '',
  });
}

/** The nodes that validation found something wrong with. */
export function invalidNodeIds(
  issues: readonly ValidationIssue[],
  workflow: Workflow,
): Set<string> {
  const ids = new Set<string>();
  for (const issue of issues) {
    if (issue.nodeId) ids.add(issue.nodeId);
    const edge = workflow.edges.find((e) => e.id === issue.edgeId);
    if (edge) {
      ids.add(edge.sourceNodeId);
      ids.add(edge.targetNodeId);
    }
    for (const id of (issue.params.nodes ?? '').split(',').filter(Boolean)) ids.add(id);
  }
  return ids;
}

/** Problems between what an agent declares and where the workflow sends it. */
const ROUTING_CODES = new Set<ValidationIssue['code']>([
  'outcome_without_route',
  'status_route_on_contract',
  'undeclared_outcome',
  'cannot_reach_end',
]);

export function isRoutingIssue(issue: ValidationIssue): boolean {
  return ROUTING_CODES.has(issue.code);
}
