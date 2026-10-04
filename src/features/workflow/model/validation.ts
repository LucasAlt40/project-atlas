import type { Translate } from '@/i18n';
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
  return t(`workflow.issue.${issue.code}`, {
    node,
    edge: edgeName,
    nodes,
    max: issue.params.max ?? '',
    target: issue.params.target ?? '',
    reason: issue.params.reason ?? '',
    outcome: issue.params.outcome ?? '',
    agent: issue.params.agent ?? '',
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
