import type { ReactNode } from 'react';
import { Button } from '@/components/ui/Button';
import { CollapsibleSection } from '@/components/ui/CollapsibleSection';
import { StateBadge } from '@/components/ui/status';
import { Icon } from '@/components/ui/Icon';
import { useI18n } from '@/i18n/I18nProvider';
import type { Translate, TranslationKey } from '@/i18n';
import type { Agent } from '@/features/agents/types';
import { shortId } from '@/features/workspace/model/inspection';
import type { ConditionDto, ConditionOperatorDto, OutcomeDto } from '@/lib/tauri/commands';
import {
  setFailureRoute,
  setLoopLimit,
  updateAgentNode,
  updateCondition,
  updateEdge,
  updateEndOutcome,
  updateNodeLabel,
} from '../model/edit';
import { conditionText } from '../model/graph';
import { handoffsOf, handoffsOver } from '../model/integration';
import { FindingText, HandoffList } from './HandoffView';
import { outcomeLabel } from '@/features/agents/model/contract';
import { NODE_STATUS_VIEW, nodeLabel } from '../model/status';
import type { NodeState, Workflow, WorkflowEdge, WorkflowNode, WorkflowRun } from '../types';
import { StepTools } from './StepTools';
import styles from './Workflow.module.css';

/** What the node's agent is, from the catalog: the node itself never copies any of it. */
export interface AgentFacts {
  name: string;
  personality: string;
  runtime: string;
  model: string;
  isolation: boolean;
  /** What the agent may conclude with (empty for a general agent). */
  outcomes: OutcomeDto[];
}

interface NodeProps {
  workflow: Workflow;
  node: WorkflowNode;
  editable: boolean;
  agents: Agent[];
  factsOf: (agentId: string) => AgentFacts | undefined;
  run: WorkflowRun | undefined;
  onChange: (change: (workflow: Workflow) => Workflow) => void;
  onOpenExecution: (executionId: string) => void;
  /**
   * Where the step's tools (MCP grants) are managed: the workspace, and the steps of the stored
   * workflow (a grant can only name a step the core already knows). Absent: no tools section.
   */
  tools?: { workspaceId: string; savedNodeIds: ReadonlySet<string> };
  /** Closes the drawer (the selection is cleared). */
  onClose?: () => void;
  /** The drawer's footer: saves the workflow. Absent: no footer. */
  save?: { dirty: boolean; onSave: () => void };
  /** Validation has nothing to say about this step. */
  valid?: boolean;
}

const KNOWN_FIELDS = [
  'result.outcome',
  'result.status',
  'result.summary',
  'result.next_action',
  'result.findings',
  'result.matched',
  'validation.status',
] as const;
const OPERATORS: readonly ConditionOperatorDto[] = ['equals', 'not_equals', 'exists', 'not_exists'];

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className={styles.field}>
      <span className={styles.fieldLabel}>{label}</span>
      {children}
    </label>
  );
}

export function ConditionEditor({
  condition,
  disabled,
  outcomes,
  onChange,
}: {
  condition: ConditionDto;
  disabled: boolean;
  /** The outcomes the source agent declares: the only values `result.outcome` can take. */
  outcomes?: readonly OutcomeDto[];
  onChange: (condition: ConditionDto) => void;
}) {
  const { t } = useI18n();
  const choosing = condition.field === 'result.outcome' && outcomes !== undefined;
  const needsValue = condition.operator === 'equals' || condition.operator === 'not_equals';
  return (
    <div className={styles.conditionEditor}>
      <Field label={t('workflow.condition.field')}>
        <select
          className={styles.control}
          value={condition.field}
          disabled={disabled}
          onChange={(e) => {
            onChange({ ...condition, field: e.target.value });
          }}
        >
          {KNOWN_FIELDS.map((field) => (
            <option key={field} value={field}>
              {field} · {t(`workflow.field.${field}` as TranslationKey)}
            </option>
          ))}
        </select>
      </Field>
      <Field label={t('workflow.condition.operator')}>
        <select
          className={styles.control}
          value={condition.operator}
          disabled={disabled}
          onChange={(e) => {
            onChange({ ...condition, operator: e.target.value as ConditionOperatorDto });
          }}
        >
          {OPERATORS.map((operator) => (
            <option key={operator} value={operator}>
              {t(`workflow.operator.${operator}` as TranslationKey)}
            </option>
          ))}
        </select>
      </Field>
      {needsValue && (
        <Field label={t('workflow.condition.value')}>
          {choosing ? (
            <select
              className={styles.control}
              value={condition.value ?? ''}
              disabled={disabled}
              onChange={(e) => {
                onChange({ ...condition, value: e.target.value });
              }}
            >
              <option value="">{t('workflow.condition.chooseOutcome')}</option>
              {outcomes.map((outcome) => (
                <option key={outcome.id} value={outcome.id}>
                  {outcome.label} ({outcome.id})
                </option>
              ))}
              {condition.value && !outcomes.some((o) => o.id === condition.value) && (
                <option value={condition.value}>{condition.value}</option>
              )}
            </select>
          ) : (
            <input
              className={styles.control}
              value={condition.value ?? ''}
              disabled={disabled}
              onChange={(e) => {
                onChange({ ...condition, value: e.target.value });
              }}
            />
          )}
        </Field>
      )}
    </div>
  );
}

/** The configuration of one node, and (when a run is shown) what became of it. */
export function NodeInspector({
  workflow,
  node,
  editable,
  agents,
  factsOf,
  run,
  onChange,
  onOpenExecution,
  tools,
  onClose,
  save,
  valid,
}: NodeProps) {
  const { t } = useI18n();
  const state = run?.nodes[node.id];
  const incoming = workflow.edges.filter((e) => e.targetNodeId === node.id);
  const outgoing = workflow.edges.filter((e) => e.sourceNodeId === node.id);
  const name = (id: string) => workflow.nodes.find((n) => n.id === id)?.label ?? id;
  const disabled = !editable;
  const facts = node.type === 'agent' ? factsOf(node.agentId) : undefined;

  const outcomes = facts?.outcomes ?? [];
  const failure =
    node.type === 'agent' && node.failurePolicy.type === 'route_to_node'
      ? t('workflow.inspector.summary.routeTo', { name: name(node.failurePolicy.nodeId) })
      : t('workflow.inspector.summary.stop');
  const loopText = node.loopPolicy
    ? t('workflow.inspector.summary.loop', { n: node.loopPolicy.maxIterations })
    : t('workflow.inspector.noLoop');
  const routesSummary = [
    incoming.length > 0
      ? t('workflow.inspector.summary.after', {
          names: incoming.map((e) => name(e.sourceNodeId)).join(', '),
        })
      : null,
    outgoing.length > 0
      ? t('workflow.inspector.summary.next', {
          names: outgoing.map((e) => name(e.targetNodeId)).join(', '),
        })
      : null,
  ]
    .filter(Boolean)
    .join(' · ');
  const section = (title: string, summary: string, open: boolean, body: ReactNode) => (
    <CollapsibleSection
      title={title}
      label={t('workflow.inspector.sectionLabel', { name: title })}
      variant="flat"
      defaultOpen={open}
      summary={summary || '—'}
    >
      {body}
    </CollapsibleSection>
  );

  return (
    <section className={styles.drawer} aria-label={t('workflow.inspector.node')}>
      <header className={styles.drawerHead}>
        <div className={styles.drawerTop}>
          <div className={styles.drawerTitles}>
            <h3 className={styles.drawerTitle}>{node.label}</h3>
            <p className={styles.muted}>
              {t(
                node.type === 'agent'
                  ? 'workflow.inspector.kind.agent'
                  : node.type === 'condition'
                    ? 'workflow.inspector.kind.condition'
                    : 'workflow.inspector.kind.end',
              )}
            </p>
          </div>
          {onClose && (
            <button
              type="button"
              className={styles.iconButton}
              aria-label={t('workflow.inspector.close')}
              title={t('workflow.inspector.close')}
              onClick={onClose}
            >
              <Icon name="close" size={18} />
            </button>
          )}
        </div>
        <div className={styles.drawerStrip}>
          {valid === undefined ? null : valid ? (
            <StateBadge state="completed">{t('workflow.inspector.stepValid')}</StateBadge>
          ) : (
            <StateBadge state="blocked">{t('workflow.inspector.stepInvalid')}</StateBadge>
          )}
          <span className={styles.nodeId} title={t('workflow.inspector.nodeId')}>
            {node.id}
          </span>
        </div>
      </header>
      <div className={styles.drawerSections}>
        {node.type === 'agent' && (
          <>
            {section(
              t('workflow.inspector.sectionAgent'),
              facts ? `${facts.name} · ${facts.runtime}` : '',
              true,
              <>
                <Field label={t('workflow.inspector.agent')}>
                  <select
                    className={styles.control}
                    value={node.agentId}
                    disabled={disabled}
                    onChange={(e) => {
                      onChange((w) => updateAgentNode(w, node.id, { agentId: e.target.value }));
                    }}
                  >
                    {node.agentId === '' && (
                      <option value="">{t('workflow.inspector.chooseAgent')}</option>
                    )}
                    {agents.map((agent) => (
                      <option key={agent.id} value={agent.id}>
                        {agent.name}
                      </option>
                    ))}
                  </select>
                </Field>
                <Field label={t('workflow.inspector.label')}>
                  <input
                    className={styles.control}
                    value={node.label}
                    disabled={disabled}
                    onChange={(e) => {
                      onChange((w) => updateNodeLabel(w, node.id, e.target.value));
                    }}
                  />
                </Field>
                {facts && (
                  <dl className={styles.facts}>
                    <div>
                      <dt>{t('workflow.inspector.personality')}</dt>
                      <dd>{facts.personality}</dd>
                    </div>
                    <div>
                      <dt>{t('workflow.inspector.runtime')}</dt>
                      <dd>{facts.runtime}</dd>
                    </div>
                    <div>
                      <dt>{t('workflow.inspector.model')}</dt>
                      <dd>{facts.model || '—'}</dd>
                    </div>
                    <div>
                      <dt>{t('workflow.inspector.isolation')}</dt>
                      <dd>
                        {facts.isolation ? t('workflow.inspector.on') : t('workflow.inspector.off')}
                      </dd>
                    </div>
                    <div>
                      <dt>{t('workflow.inspector.context')}</dt>
                      <dd>{t('workflow.inspector.taskAware')}</dd>
                    </div>
                  </dl>
                )}
              </>,
            )}
            {section(
              t('workflow.inspector.sectionContract'),
              outcomes.length === 0
                ? t('workflow.inspector.resultGeneral')
                : outcomes.map((o) => o.id).join(', '),
              false,
              <p className={styles.outcomes}>
                {outcomes.length === 0
                  ? t('workflow.inspector.resultGeneral')
                  : outcomes.map((o, i) => (
                      <span key={o.id} className={styles.outcomeChip} data-first={i === 0}>
                        {o.id}
                      </span>
                    ))}
              </p>,
            )}
            {section(
              t('workflow.inspector.sectionInstructions'),
              node.instructions.trim() || t('workflow.inspector.summary.empty'),
              false,
              <>
                <p className={styles.muted}>
                  {t('workflow.inspector.tokens', {
                    n: Math.ceil((node.instructions.length + node.completionCriteria.length) / 4),
                  })}
                </p>
                <Field label={t('workflow.inspector.instructions')}>
                  <textarea
                    className={styles.textarea}
                    value={node.instructions}
                    disabled={disabled}
                    placeholder={t('workflow.inspector.instructionsPlaceholder')}
                    onChange={(e) => {
                      onChange((w) =>
                        updateAgentNode(w, node.id, { instructions: e.target.value }),
                      );
                    }}
                  />
                </Field>
                <Field label={t('workflow.inspector.completionCriteria')}>
                  <textarea
                    className={styles.textarea}
                    value={node.completionCriteria}
                    disabled={disabled}
                    placeholder={t('workflow.inspector.completionCriteriaPlaceholder')}
                    onChange={(e) => {
                      const completionCriteria = e.target.value;
                      onChange((w) => updateAgentNode(w, node.id, { completionCriteria }));
                    }}
                  />
                  <p className={styles.muted}>{t('workflow.inspector.completionCriteriaHint')}</p>
                </Field>
              </>,
            )}
            {section(
              t('workflow.inspector.sectionPermissionsOnly'),
              node.executionPolicy.readOnly
                ? t('workflow.inspector.summary.readOnly')
                : t('workflow.inspector.summary.writes'),
              false,
              <>
                <div className={styles.field}>
                  <label className={[styles.checkRow, styles.switchRow].join(' ')}>
                    <input
                      className={styles.switch}
                      type="checkbox"
                      checked={node.executionPolicy.readOnly}
                      disabled={disabled}
                      onChange={(e) => {
                        const readOnly = e.target.checked;
                        onChange((w) =>
                          updateAgentNode(w, node.id, {
                            executionPolicy: { ...node.executionPolicy, readOnly },
                          }),
                        );
                      }}
                    />{' '}
                    {t('workflow.inspector.readOnly')}
                  </label>
                  <p className={styles.muted}>{t('workflow.inspector.readOnlyHint')}</p>
                </div>
                <Field label={t('workflow.inspector.integrations')}>
                  <label className={styles.checkRow}>
                    <input
                      type="checkbox"
                      checked={node.executionPolicy.announceIntegrations}
                      disabled={disabled}
                      onChange={(e) => {
                        const announceIntegrations = e.target.checked;
                        onChange((w) =>
                          updateAgentNode(w, node.id, {
                            executionPolicy: { ...node.executionPolicy, announceIntegrations },
                          }),
                        );
                      }}
                    />{' '}
                    {t('workflow.inspector.announceIntegrations')}
                  </label>
                  <p className={styles.muted}>{t('workflow.inspector.announceIntegrationsHint')}</p>
                </Field>
              </>,
            )}
            {section(
              t('workflow.inspector.sectionResilienceOnly'),
              t('workflow.inspector.summary.resilience', {
                retries: node.retryPolicy.maxRetries,
                failure,
                loop: loopText,
              }),
              false,
              <>
                <Field label={t('workflow.inspector.retries')}>
                  <input
                    className={styles.control}
                    type="number"
                    min={0}
                    max={5}
                    value={node.retryPolicy.maxRetries}
                    disabled={disabled}
                    onChange={(e) => {
                      const value = Math.min(5, Math.max(0, Number(e.target.value) || 0));
                      onChange((w) =>
                        updateAgentNode(w, node.id, { retryPolicy: { maxRetries: value } }),
                      );
                    }}
                  />
                </Field>
                <Field label={t('workflow.inspector.onFailure')}>
                  <select
                    className={styles.control}
                    value={
                      node.failurePolicy.type === 'route_to_node' ? node.failurePolicy.nodeId : ''
                    }
                    disabled={disabled}
                    onChange={(e) => {
                      onChange((w) => setFailureRoute(w, node.id, e.target.value || null));
                    }}
                  >
                    <option value="">{t('workflow.inspector.stopWorkflow')}</option>
                    {workflow.nodes
                      .filter((n) => n.id !== node.id && n.type !== 'end')
                      .map((n) => (
                        <option key={n.id} value={n.id}>
                          {t('workflow.inspector.routeTo', { name: n.label })}
                        </option>
                      ))}
                  </select>
                </Field>
                <Field label={t('workflow.inspector.maxLoop')}>
                  <input
                    className={styles.control}
                    type="number"
                    min={1}
                    max={20}
                    value={node.loopPolicy?.maxIterations ?? ''}
                    placeholder={t('workflow.inspector.noLoop')}
                    disabled={disabled}
                    onChange={(e) => {
                      const value =
                        e.target.value === ''
                          ? null
                          : Math.min(20, Math.max(1, Number(e.target.value) || 1));
                      onChange((w) => setLoopLimit(w, node.id, value));
                    }}
                  />
                </Field>
              </>,
            )}
            {tools &&
              editable &&
              section(
                t('workflow.inspector.sectionTools'),
                t('workflow.inspector.summary.tools'),
                false,
                <>
                  <StepTools
                    workspaceId={tools.workspaceId}
                    workflow={workflow}
                    node={node}
                    saved={tools.savedNodeIds.has(node.id)}
                  />
                </>,
              )}
          </>
        )}

        {node.type === 'condition' &&
          section(
            t('workflow.inspector.sectionCondition'),
            '',
            true,
            <>
              <Field label={t('workflow.inspector.label')}>
                <input
                  className={styles.control}
                  value={node.label}
                  disabled={disabled}
                  onChange={(e) => {
                    onChange((w) => updateNodeLabel(w, node.id, e.target.value));
                  }}
                />
              </Field>
              <ConditionEditor
                condition={node.condition}
                disabled={disabled}
                onChange={(condition) => {
                  onChange((w) => updateCondition(w, node.id, condition));
                }}
              />

              <Field label={t('workflow.inspector.maxLoop')}>
                <input
                  className={styles.control}
                  type="number"
                  min={1}
                  max={20}
                  value={node.loopPolicy?.maxIterations ?? ''}
                  placeholder={t('workflow.inspector.noLoop')}
                  disabled={disabled}
                  onChange={(e) => {
                    const value =
                      e.target.value === ''
                        ? null
                        : Math.min(20, Math.max(1, Number(e.target.value) || 1));
                    onChange((w) => setLoopLimit(w, node.id, value));
                  }}
                />
              </Field>
            </>,
          )}

        {node.type === 'end' &&
          section(
            t('workflow.inspector.sectionEnd'),
            '',
            true,
            <>
              <Field label={t('workflow.inspector.label')}>
                <input
                  className={styles.control}
                  value={node.label}
                  disabled={disabled}
                  onChange={(e) => {
                    onChange((w) => updateNodeLabel(w, node.id, e.target.value));
                  }}
                />
              </Field>
              {node.type === 'end' && (
                <Field label={t('workflow.inspector.outcome')}>
                  <select
                    className={styles.control}
                    value={node.outcome}
                    disabled={disabled}
                    onChange={(e) => {
                      onChange((w) =>
                        updateEndOutcome(
                          w,
                          node.id,
                          e.target.value as 'done' | 'failed' | 'cancelled',
                        ),
                      );
                    }}
                  >
                    {(['done', 'failed', 'cancelled'] as const).map((outcome) => (
                      <option key={outcome} value={outcome}>
                        {t(`workflow.end.${outcome}` as TranslationKey)}
                      </option>
                    ))}
                  </select>
                </Field>
              )}
            </>,
          )}

        {section(
          t('workflow.inspector.sectionRoutes'),
          routesSummary,
          false,
          <div className={styles.routes}>
            <h4>{t('workflow.inspector.dependencies')}</h4>
            {incoming.length === 0 ? (
              <p className={styles.muted}>{t('workflow.inspector.noDependencies')}</p>
            ) : (
              <ul>
                {incoming.map((edge) => (
                  <li key={edge.id}>{name(edge.sourceNodeId)}</li>
                ))}
              </ul>
            )}
            <h4>{t('workflow.inspector.routes')}</h4>
            {outgoing.length === 0 ? (
              <p className={styles.muted}>{t('workflow.inspector.noRoutes')}</p>
            ) : (
              <ul>
                {outgoing.map((edge) => (
                  <li key={edge.id}>
                    {name(edge.targetNodeId)}
                    {edge.condition && (
                      <span className={styles.muted}> — {conditionText(edge.condition)}</span>
                    )}
                  </li>
                ))}
              </ul>
            )}
          </div>,
        )}

        {state && (
          <div className={styles.routes}>
            <h4>{t('workflow.inspector.execution')}</h4>
            <p>
              <span aria-hidden="true">{NODE_STATUS_VIEW[state.status].symbol}</span>{' '}
              {t(NODE_STATUS_VIEW[state.status].label)}
            </p>
            {outcomeOf(state) && (
              <p>
                {t('workflow.inspector.outcomeIs')}{' '}
                <strong data-outcome={outcomeOf(state)}>
                  {outcomeLabel(facts?.outcomes ?? [], outcomeOf(state) ?? '').toUpperCase()}
                </strong>
              </p>
            )}
            <NodeFindings run={run} nodeId={node.id} />
            {state.reason && state.status !== 'completed' && (
              <p className={styles.muted}>{reasonText(t, state.reason)}</p>
            )}
            <NodeHandoffs run={run} nodeId={node.id} onOpenExecution={onOpenExecution} />
            {state.attempts.length > 0 && (
              <ul className={styles.attempts}>
                {state.attempts.map((attempt) => (
                  <li key={attempt.executionId}>
                    <button
                      type="button"
                      className={styles.link}
                      onClick={() => {
                        onOpenExecution(attempt.executionId);
                      }}
                    >
                      {t('workflow.inspector.openExecution', {
                        id: shortId(attempt.executionId),
                        attempt: attempt.attempt,
                      })}
                    </button>{' '}
                    <span className={styles.muted}>
                      {t(`workflow.attempt.${attempt.status}` as TranslationKey)}
                    </span>
                  </li>
                ))}
              </ul>
            )}
            <p className={styles.muted}>
              {t('workflow.inspector.snapshot', {
                name: nodeLabel(run, node.id),
                version: run.workflowVersion,
              })}
            </p>
          </div>
        )}
      </div>
      {save && editable && (
        <footer className={styles.drawerFoot}>
          <span className={styles.saveState} data-dirty={save.dirty}>
            {save.dirty ? t('workflow.inspector.unsaved') : t('workflow.inspector.saved')}
          </span>
          <Button disabled={!save.dirty} onClick={save.onSave}>
            {t('workflow.inspector.save')}
          </Button>
        </footer>
      )}
    </section>
  );
}

/** The outcome of the node's latest completed attempt, if its agent declared one. */
function outcomeOf(state: NodeState): string | null {
  const done = state.attempts.filter((a) => a.status === 'completed');
  return done.length > 0 ? (done[done.length - 1]?.outcome ?? null) : null;
}

/** What the node's step found, as its validation result keeps it. */
function NodeFindings({ run, nodeId }: { run: WorkflowRun; nodeId: string }) {
  const { t } = useI18n();
  const entry = run.state.validationResults.find((v) => v.nodeId === nodeId);
  if (!entry || entry.findings.length === 0) return null;
  return (
    <>
      <h5>{t('handoff.findings')}</h5>
      <ul>
        {entry.findings.map((f, index) => (
          <li key={index}>
            <FindingText finding={f} />
          </li>
        ))}
      </ul>
    </>
  );
}

const KNOWN_REASONS = [
  'interrupted',
  'max_iterations',
  'dependency_failed',
  'skipped_by_route',
  'not_reached',
  'end_done',
  'end_failed',
  'end_cancelled',
] as const;

/** A reason is a stable code the core gave, or (for a failed step) the failure's own message. */
function reasonText(t: Translate, reason: string): string {
  const known = KNOWN_REASONS.find((code) => code === reason);
  return known ? t(`workflow.reason.${known}` as TranslationKey) : reason;
}

/** Which road an edge is, as the editor offers it: always, one declared outcome, or custom. */
type Preset = 'always' | 'custom' | `outcome:${string}` | 'pass' | 'fail';

function presetOf(edge: WorkflowEdge, outcomes: readonly OutcomeDto[]): Preset {
  const c = edge.condition;
  if (!c) return 'always';
  if (c.operator === 'equals' && c.field === 'result.outcome') {
    const declared = outcomes.find((o) => o.id === c.value);
    if (declared) return `outcome:${declared.id}`;
  }
  // The older way, for agents that declare no outcomes.
  if (outcomes.length === 0 && c.field === 'result.status' && c.operator === 'equals') {
    if (c.value === 'pass') return 'pass';
    if (c.value === 'fail') return 'fail';
  }
  return 'custom';
}

export function EdgeInspector({
  workflow,
  edge,
  editable,
  outcomesOf,
  agentName,
  onChange,
}: {
  workflow: Workflow;
  edge: WorkflowEdge;
  editable: boolean;
  /** What the agent of a node may conclude with. */
  outcomesOf: (nodeId: string) => OutcomeDto[];
  agentName: (nodeId: string) => string;
  onChange: (change: (workflow: Workflow) => Workflow) => void;
}) {
  const { t } = useI18n();
  const name = (id: string) => workflow.nodes.find((n) => n.id === id)?.label ?? id;
  const outcomes = outcomesOf(edge.sourceNodeId);
  const preset = presetOf(edge, outcomes);
  const apply = (next: Preset) => {
    const labelOf = (id: string) => outcomes.find((o) => o.id === id)?.label ?? id;
    let condition: ConditionDto | null;
    let label = edge.label;
    if (next === 'always') {
      condition = null;
      label = '';
    } else if (next === 'custom') {
      condition = edge.condition ?? {
        field: 'result.outcome',
        operator: 'equals',
        value: '',
      };
    } else if (next === 'pass' || next === 'fail') {
      condition = { field: 'result.status', operator: 'equals', value: next };
      label = next;
    } else {
      const id = next.slice('outcome:'.length);
      condition = { field: 'result.outcome', operator: 'equals', value: id };
      label = labelOf(id);
    }
    onChange((w) => updateEdge(w, edge.id, { condition, label }));
  };
  const options: { value: Preset; text: string }[] = [
    { value: 'always', text: t('workflow.edge.always') },
    ...(outcomes.length > 0
      ? outcomes.map((o) => ({ value: `outcome:${o.id}` as Preset, text: `${o.label} (${o.id})` }))
      : [
          { value: 'pass' as Preset, text: t('workflow.edge.pass') },
          { value: 'fail' as Preset, text: t('workflow.edge.fail') },
        ]),
    { value: 'custom', text: t('workflow.edge.custom') },
  ];
  const c = edge.condition;
  const undeclared =
    c?.field === 'result.outcome' &&
    (c.operator === 'equals' || c.operator === 'not_equals') &&
    !!c.value &&
    !outcomes.some((o) => o.id.toLowerCase() === c.value?.toLowerCase());
  return (
    <section className={styles.inspector} aria-label={t('workflow.inspector.edge')}>
      <h3 className={styles.inspectorTitle}>
        {name(edge.sourceNodeId)} → {name(edge.targetNodeId)}
      </h3>
      <Field label={t('workflow.inspector.edgeWhen')}>
        <select
          className={styles.control}
          value={preset}
          disabled={!editable}
          onChange={(e) => {
            apply(e.target.value as Preset);
          }}
        >
          {options.map((option) => (
            <option key={option.value} value={option.value}>
              {option.text}
            </option>
          ))}
        </select>
      </Field>
      {preset === 'custom' && edge.condition && (
        <ConditionEditor
          condition={edge.condition}
          disabled={!editable}
          outcomes={outcomes}
          onChange={(condition) => {
            onChange((w) => updateEdge(w, edge.id, { condition }));
          }}
        />
      )}
      {undeclared && (
        <p role="alert" className={styles.failure}>
          {t('workflow.edge.undeclaredOutcome', {
            outcome: c.value ?? '',
            agent: agentName(edge.sourceNodeId),
          })}
        </p>
      )}
      <Field label={t('workflow.inspector.edgeLabel')}>
        <input
          className={styles.control}
          value={edge.label}
          disabled={!editable}
          onChange={(e) => {
            onChange((w) => updateEdge(w, edge.id, { label: e.target.value }));
          }}
        />
      </Field>
    </section>
  );
}

/** What the node was handed (its input) and what it handed on (its output), kept out of the way
 * until asked for. */
function NodeHandoffs({
  run,
  nodeId,
  onOpenExecution,
}: {
  run: WorkflowRun;
  nodeId: string;
  onOpenExecution: (executionId: string) => void;
}) {
  const { t } = useI18n();
  const { received, sent } = handoffsOf(run, nodeId);
  return (
    <>
      <details className={styles.fold}>
        <summary>{t('handoff.input', { n: received.length })}</summary>
        <HandoffList run={run} handoffs={received} onOpenExecution={onOpenExecution} />
      </details>
      <details className={styles.fold}>
        <summary>{t('handoff.output', { n: sent.length })}</summary>
        <HandoffList run={run} handoffs={sent} onOpenExecution={onOpenExecution} />
      </details>
    </>
  );
}

/** A connection of a run, as the handoffs that travelled over it. */
export function LinkInspector({
  run,
  linkId,
  onOpenExecution,
}: {
  run: WorkflowRun;
  linkId: string;
  onOpenExecution: (executionId: string) => void;
}) {
  const { t } = useI18n();
  const edge = run.workflow.edges.find((e) => e.id === linkId);
  const from = edge?.sourceNodeId ?? linkId.replace(/^failure:/, '');
  const handoffs = handoffsOver(run, linkId);
  const to = edge?.targetNodeId ?? handoffs[0]?.toNodeId ?? '';
  const label = (id: string) => run.workflow.nodes.find((n) => n.id === id)?.label ?? id;
  return (
    <section className={styles.inspector} aria-label={t('handoff.panel')}>
      <h3 className={styles.inspectorTitle}>{t('handoff.panelTitle')}</h3>
      <p>
        <strong>
          {label(from)}
          {to ? ` → ${label(to)}` : ''}
        </strong>
      </p>
      <HandoffList run={run} handoffs={handoffs} onOpenExecution={onOpenExecution} />
    </section>
  );
}
