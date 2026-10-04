import { useEffect, useMemo, useState } from 'react';
import { ReactFlowProvider } from '@xyflow/react';
import { useNavigation } from '@/app/NavigationContext';
import { Button } from '@/components/ui/Button';
import { useCatalog } from '@/features/agents/hooks/useCatalog';
import { ExecutionInspector } from '@/features/workspace/components/ExecutionInspector';
import { useWorkspace } from '@/features/workspace/hooks/WorkspaceProvider';
import { runKey } from '@/features/workspace/model/agentRuns';
import type { StoredExecution } from '@/features/workspace/types';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import { errorMessage } from '@/i18n/messages';
import type { PositionDto } from '@/lib/tauri/commands';
import { ChangesModal } from '../components/ChangesModal';
import { InteractionPanel } from '../components/InteractionPanel';
import { NewWorkflowPanel, type NewWorkflowChoice } from '../components/NewWorkflowPanel';
import { WorkflowCanvas, type Selection } from '../components/WorkflowCanvas';
import {
  EdgeInspector,
  LinkInspector,
  NodeInspector,
  type AgentFacts,
} from '../components/WorkflowInspector';
import { EditorOverview, RunOverview } from '../components/WorkflowOverview';
import { WorkflowToolbar } from '../components/WorkflowToolbar';
import { useWorkflows } from '../hooks/useWorkflows';
import { connect, disconnect, removeNode, resetLayout } from '../model/edit';
import { withPositions, type AgentLabel } from '../model/graph';
import { RUN_STATUS_LABEL } from '../model/status';
import { invalidNodeIds } from '../model/validation';
import { isActiveRun, type WorkflowRun } from '../types';
import styles from '../components/Workflow.module.css';

/** The question an execution asked, as its run kept it (to show it, and its answer, inspected). */
function askedBy(run: WorkflowRun | undefined, executionId: string) {
  const asked = run?.interactions.find((i) => i.executionId === executionId);
  return asked ? { asked } : {};
}

/**
 * The Workflow area of a workspace: a task becomes a graph of agents (chosen for the user in
 * Automatic mode, composed by them in Custom mode), which the core runs. The page only shows
 * and edits the definition and follows the run; it decides nothing about how it executes.
 */
export function WorkflowPage() {
  const { t } = useI18n();
  const { catalog, runtimes } = useCatalog();
  const workspace = useWorkspace();
  const { intent } = useNavigation();
  const active = workspace.active;
  const space = useWorkflows(active?.id);

  const [selection, setSelection] = useState<Selection>(null);
  const [task, setTask] = useState('');
  const [connecting, setConnecting] = useState(false);
  const [creating, setCreating] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [inspected, setInspected] = useState<StoredExecution | null>(null);
  const [reviewing, setReviewing] = useState(false);

  const agents = useMemo(() => (catalog.status === 'ready' ? catalog.agents : []), [catalog]);
  const personalities = useMemo(
    () => (catalog.status === 'ready' ? catalog.personalities : []),
    [catalog],
  );
  const labels = useMemo(() => {
    const map = new Map<string, AgentLabel>();
    for (const agent of agents) {
      map.set(agent.id, {
        name: agent.name,
        personality: personalities.find((p) => p.id === agent.personalityId)?.name ?? '',
      });
    }
    return map;
  }, [agents, personalities]);

  // Opened from an execution's breadcrumb: show that run.
  const { open, showRun } = space;
  useEffect(() => {
    if (intent?.type === 'open-workflow' && space.load.status === 'ready') {
      open(intent.workflowId);
      showRun(intent.executionId ?? null);
    }
    // Only when the intent (or the load) changes, not on every render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [intent, space.load.status]);

  // Whatever was selected belongs to the workflow it was selected in.
  const viewKey = `${space.selected?.id ?? ''}:${space.shownRun?.id ?? ''}`;
  const [shownKey, setShownKey] = useState(viewKey);
  if (shownKey !== viewKey) {
    setShownKey(viewKey);
    setSelection(null);
    setConnecting(false);
    setConfirmingDelete(false);
  }

  // What is drawn: the snapshot of the run being shown, or the draft being edited.
  const shownRun = space.shownRun;
  const workflow = shownRun ? shownRun.workflow : space.draft;
  const editable = !shownRun && !space.activeRun;
  const validation = space.validation;
  const invalid = useMemo<ReadonlySet<string>>(
    () =>
      workflow && validation && !shownRun
        ? invalidNodeIds(validation.issues, workflow)
        : new Set<string>(),
    [workflow, validation, shownRun],
  );

  if (!active) return <p className={styles.muted}>{t('workflow.noWorkspace')}</p>;
  if (catalog.status === 'loading' || space.load.status === 'loading') {
    return <p className={styles.muted}>{t('common.loadingCore')}</p>;
  }
  if (space.load.status === 'error') {
    return (
      <p role="alert" className={styles.failure}>
        {t('common.coreUnavailable', { message: errorMessage(t, space.load.error) })}
      </p>
    );
  }

  const runtimeName = (runtimeId: string) =>
    runtimes.status === 'ready'
      ? (runtimes.runtimes.find((r) => r.runtime.id === runtimeId)?.runtime.name ?? runtimeId)
      : runtimeId;
  const factsOf = (agentId: string): AgentFacts | undefined => {
    const agent = agents.find((a) => a.id === agentId);
    if (!agent) return undefined;
    return {
      name: agent.name,
      personality: labels.get(agent.id)?.personality ?? '',
      runtime: runtimeName(agent.runtimeId),
      model: agent.modelId,
      isolation: agent.worktreeIsolation,
      outcomes: agent.resultContract.outcomes,
    };
  };
  const agentName = (agentId: string) => agents.find((a) => a.id === agentId)?.name ?? agentId;

  const create = (choice: NewWorkflowChoice) => {
    void space.create(choice.templateId, choice.mode, choice.name).then((created) => {
      if (!created) return;
      setCreating(false);
      if (choice.task) setTask(choice.task);
      setNotice(
        created.missing.length > 0
          ? t('workflow.missingAgents', {
              roles: created.missing
                .map((role) => t(`workflow.role.${role}` as TranslationKey))
                .join(', '),
            })
          : null,
      );
    });
  };

  if (space.workflows.length === 0 || creating) {
    return (
      <section className={styles.page}>
        <header className={styles.header}>
          <h1 className={styles.title}>{t('workflow.title')}</h1>
        </header>
        <NewWorkflowPanel
          templates={space.templates}
          recommend={space.recommend}
          error={space.error === undefined ? null : errorMessage(t, space.error)}
          onCreate={create}
          {...(space.workflows.length > 0
            ? {
                onCancel: () => {
                  setCreating(false);
                },
              }
            : {})}
        />
      </section>
    );
  }

  const onSelect = (next: Selection) => {
    if (connecting && editable && selection?.kind === 'node' && next?.kind === 'node') {
      const from = selection.id;
      space.edit((w) => connect(w, from, next.id));
      setConnecting(false);
      return;
    }
    setSelection(next);
  };

  const openExecution = (executionId: string) => {
    const stored = workspace.history.find((execution) => execution.id === executionId);
    if (stored) {
      setInspected(stored);
      setNotice(null);
    } else {
      setNotice(t('workflow.executionNotStored'));
    }
  };

  const selectedNode =
    selection?.kind === 'node' ? workflow?.nodes.find((n) => n.id === selection.id) : undefined;
  const selectedEdge =
    selection?.kind === 'edge' ? workflow?.edges.find((e) => e.id === selection.id) : undefined;

  const runStatus = shownRun?.status;
  const canRun = editable && task.trim() !== '' && (space.validation?.valid ?? false);
  const inspectedAgent = inspected ? agents.find((a) => a.id === inspected.agentId) : undefined;

  return (
    <section className={styles.page}>
      <header className={styles.header}>
        <div className={styles.headerLeft}>
          <h1 className={styles.title}>{t('workflow.title')}</h1>
          <select
            className={styles.control}
            aria-label={t('workflow.select')}
            value={space.selected?.id ?? ''}
            onChange={(e) => {
              space.open(e.target.value);
            }}
          >
            {space.workflows.map((w) => (
              <option key={w.id} value={w.id}>
                {w.name}
              </option>
            ))}
          </select>
          <Button
            onClick={() => {
              setCreating(true);
            }}
          >
            {t('workflow.new')}
          </Button>
          {workflow && (
            <span className={styles.modeBadge} data-mode={workflow.mode}>
              {t(`workflow.mode.${workflow.mode}` as TranslationKey)}
            </span>
          )}
          {workflow && (
            <span className={styles.muted}>
              {t('workflow.version', { version: workflow.version })}
            </span>
          )}
          {runStatus && (
            <span
              className={styles.runBadge}
              data-status={runStatus}
              role="status"
              aria-label={t('workflow.runStatus', { status: t(RUN_STATUS_LABEL[runStatus]) })}
            >
              {t(RUN_STATUS_LABEL[runStatus])}
            </span>
          )}
        </div>
        <div className={styles.headerRight}>
          {space.runs.length > 0 && (
            <select
              className={styles.control}
              aria-label={t('workflow.runs')}
              value={shownRun?.id ?? ''}
              onChange={(e) => {
                space.showRun(e.target.value || null);
              }}
            >
              <option value="">{t('workflow.runs.editor')}</option>
              {space.runs.map((run) => (
                <option key={run.id} value={run.id}>
                  {new Date(run.startedAt).toLocaleString()} · {t(RUN_STATUS_LABEL[run.status])}
                </option>
              ))}
            </select>
          )}
          {workflow?.mode === 'automatic' && editable && (
            <Button onClick={space.customize}>{t('workflow.customize')}</Button>
          )}
          {editable && (
            <>
              <Button disabled={!space.dirty} onClick={() => void space.save()}>
                {t('common.save')}
              </Button>
              {confirmingDelete ? (
                <Button
                  onClick={() => {
                    setConfirmingDelete(false);
                    void space.remove();
                  }}
                >
                  {t('common.confirmDelete')}
                </Button>
              ) : (
                <Button
                  onClick={() => {
                    setConfirmingDelete(true);
                  }}
                >
                  {t('workflow.deleteWorkflow')}
                </Button>
              )}
            </>
          )}
          {shownRun && !isActiveRun(shownRun) && !space.activeRun && (
            <Button
              onClick={() => {
                space.showRun(null);
              }}
            >
              {t('workflow.backToEditor')}
            </Button>
          )}
          {runStatus === 'running' && (
            <Button onClick={() => void space.pause()}>{t('workflow.pause')}</Button>
          )}
          {(runStatus === 'paused' || runStatus === 'interrupted') && (
            <Button onClick={() => void space.resume()}>{t('workflow.resume')}</Button>
          )}
          {runStatus === 'interrupted' && shownRun && (
            <Button
              onClick={() => {
                void space.cancel().then(() => space.run(shownRun.task));
              }}
            >
              {t('workflow.restart')}
            </Button>
          )}
          {shownRun && isActiveRun(shownRun) && (
            <Button onClick={() => void space.cancel()}>{t('workflow.cancel')}</Button>
          )}
        </div>
      </header>

      {editable && (
        <div className={styles.runBar}>
          <input
            className={styles.control}
            aria-label={t('workflow.task')}
            placeholder={t('workflow.task.placeholder')}
            value={task}
            onChange={(e) => {
              setTask(e.target.value);
            }}
          />
          <Button
            disabled={!canRun}
            onClick={() => {
              void space.run(task.trim());
            }}
          >
            {t('workflow.run')}
          </Button>
        </div>
      )}

      {space.error !== undefined && (
        <p role="alert" className={styles.failure}>
          {errorMessage(t, space.error)}
        </p>
      )}
      {notice && (
        <p role="status" className={styles.warning}>
          {notice}
        </p>
      )}
      {shownRun?.interactions
        .filter((interaction) => interaction.status === 'pending')
        .map((interaction) => (
          <InteractionPanel
            key={interaction.id}
            interaction={interaction}
            busy={space.answering}
            error={
              space.answerError?.id === interaction.id
                ? errorMessage(t, space.answerError.error)
                : null
            }
            onAnswer={(reply) => {
              void space.answer(interaction.id, reply);
            }}
            onCancelRun={() => void space.cancel()}
          />
        ))}

      {workflow && (
        <ReactFlowProvider>
          <WorkflowToolbar
            editable={editable}
            agents={agents}
            hasSelection={selection !== null}
            canConnect={selection?.kind === 'node'}
            connecting={connecting}
            canUndo={space.canUndo}
            canRedo={space.canRedo}
            onAddAgent={(agent) => {
              space.addAgent(agent.id, agent.name);
            }}
            onAddCondition={() => {
              space.addCondition(t('workflow.node.condition'));
            }}
            onAddEnd={() => {
              space.addEnd(t('workflow.end.done'));
            }}
            onToggleConnect={() => {
              setConnecting((value) => !value);
            }}
            onDelete={() => {
              if (selection?.kind === 'node') {
                const id = selection.id;
                space.edit((w) => removeNode(w, id));
              } else if (selection?.kind === 'edge') {
                const id = selection.id;
                space.edit((w) => disconnect(w, id));
              }
              setSelection(null);
            }}
            onUndo={space.undo}
            onRedo={space.redo}
            onResetLayout={() => {
              space.edit(resetLayout);
            }}
          />
          <div className={styles.workspace}>
            <WorkflowCanvas
              workflow={workflow}
              run={shownRun}
              agents={labels}
              invalidNodes={invalid}
              editable={editable}
              selection={selection}
              onSelect={onSelect}
              onMove={(positions: Map<string, PositionDto>) => {
                space.edit((w) => withPositions(w, positions));
              }}
              onConnect={(source, target) => {
                space.edit((w) => connect(w, source, target));
              }}
              onDeleteNode={(id) => {
                space.edit((w) => removeNode(w, id));
                setSelection(null);
              }}
              onDeleteEdge={(id) => {
                space.edit((w) => disconnect(w, id));
                setSelection(null);
              }}
            />
            <aside className={styles.side} aria-label={t('workflow.side')}>
              {selectedNode ? (
                <NodeInspector
                  workflow={workflow}
                  node={selectedNode}
                  editable={editable}
                  agents={agents}
                  factsOf={factsOf}
                  run={shownRun}
                  onChange={space.edit}
                  onOpenExecution={openExecution}
                />
              ) : shownRun && selection?.kind === 'edge' ? (
                <LinkInspector
                  run={shownRun}
                  linkId={selection.id}
                  onOpenExecution={openExecution}
                />
              ) : selectedEdge ? (
                <EdgeInspector
                  workflow={workflow}
                  edge={selectedEdge}
                  editable={editable}
                  outcomesOf={(nodeId) => {
                    const source = workflow.nodes.find((n) => n.id === nodeId);
                    return source?.type === 'agent'
                      ? (factsOf(source.agentId)?.outcomes ?? [])
                      : [];
                  }}
                  agentName={(nodeId) => {
                    const source = workflow.nodes.find((n) => n.id === nodeId);
                    return source?.type === 'agent' ? agentName(source.agentId) : '';
                  }}
                  onChange={space.edit}
                />
              ) : shownRun ? (
                <RunOverview
                  run={shownRun}
                  agentName={agentName}
                  onOpenExecution={openExecution}
                  ides={space.ides}
                  busy={space.integrating}
                  recovery={space.recovery}
                  onResume={() => void space.resume()}
                  code={{
                    review: () => {
                      setReviewing(true);
                    },
                    openInIde: (ideId) => {
                      void space.openRunInIde(ideId);
                    },
                    apply: () => void space.integrate('apply'),
                    keep: () => void space.integrate('keep'),
                    discard: () => void space.integrate('discard'),
                  }}
                />
              ) : (
                <EditorOverview
                  workflow={workflow}
                  validation={space.validation}
                  editable={editable}
                  onRename={(name) => {
                    space.edit((w) => ({ ...w, name }));
                  }}
                />
              )}
            </aside>
          </div>
        </ReactFlowProvider>
      )}

      {reviewing && shownRun && (
        <ChangesModal
          run={shownRun}
          onClose={() => {
            setReviewing(false);
          }}
        />
      )}
      {inspected && (
        <ExecutionInspector
          execution={inspected}
          messages={workspace.conversations.messages[runKey(active.id, inspected.agentId)] ?? []}
          context={{
            workspaceName: active.name,
            agentName: inspectedAgent?.name ?? inspected.agentId,
            personalityName: inspectedAgent
              ? (labels.get(inspectedAgent.id)?.personality ?? '')
              : '',
            runtimeName: runtimeName(inspected.runtimeId),
            capabilities:
              runtimes.status === 'ready'
                ? runtimes.runtimes.find((r) => r.runtime.id === inspected.runtimeId)?.runtime
                    .capabilities
                : undefined,
            worktreeIsolation: inspectedAgent?.worktreeIsolation ?? false,
          }}
          {...askedBy(shownRun, inspected.id)}
          onClose={() => {
            setInspected(null);
          }}
        />
      )}
    </section>
  );
}
