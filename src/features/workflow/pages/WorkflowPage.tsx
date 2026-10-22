import { useEffect, useMemo, useState } from 'react';
import { ReactFlowProvider } from '@xyflow/react';
import { useNavigation } from '@/app/NavigationContext';
import { Callout } from '@/components/ui/Callout';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { OverflowMenu } from '@/components/ui/OverflowMenu';
import { PageHeader } from '@/components/ui/PageHeader';
import { StateBadge } from '@/components/ui/status';
import { Icon } from '@/components/ui/Icon';
import { useCatalog } from '@/features/agents/hooks/useCatalog';
import { ExportDialog, ImportDialog } from '@/features/portability';
import { ExecutionInspector } from '@/features/workspace/components/ExecutionInspector';
import { useWorkspace } from '@/features/workspace/hooks/WorkspaceProvider';
import { ScopePicker } from '@/features/workspace/components/ScopePicker';
import {
  normalizeChoice,
  toScopeRequest,
  type ScopeChoice,
} from '@/features/workspace/model/scope';
import { runKey } from '@/features/workspace/model/agentRuns';
import type { StoredExecution } from '@/features/workspace/types';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import { errorMessage } from '@/i18n/messages';
import type { ImportReportDto, PositionDto } from '@/lib/tauri/commands';
import { ChangesModal } from '../components/ChangesModal';
import { InteractionPanel } from '../components/InteractionPanel';
import { LiveWorkspacePanel } from '../components/LiveWorkspacePanel';
import { ReviewWorkspace } from '../components/ReviewWorkspace';
import { RouteRepairDialog } from '../components/RouteRepairDialog';
import { SpecRunPanel } from '@/features/specs/components/SpecRunPanel';
import { StartChoice, VIBE, type StartFrom } from '@/features/specs/components/StartChoice';
import { useReadySpecs } from '@/features/specs/hooks/useReadySpecs';
import { NewWorkflowPanel, type NewWorkflowChoice } from '../components/NewWorkflowPanel';
import { AutoPlansToggle } from '../components/AutoPlansToggle';
import { SaveTemplate } from '../components/SaveTemplate';
import { loadAutoPlans, saveAutoPlans } from '../model/planPreference';
import { WorkflowCanvas, type Selection } from '../components/WorkflowCanvas';
import {
  EdgeInspector,
  LinkInspector,
  NodeInspector,
  type AgentFacts,
} from '../components/WorkflowInspector';
import { EditorOverview, RunOverview } from '../components/WorkflowOverview';
import { HistoryTools, NodeTools, ZoomTools } from '../components/WorkflowToolbar';
import { useWorkflows } from '../hooks/useWorkflows';
import { connect, disconnect, removeNode, resetLayout } from '../model/edit';
import { withPositions, type AgentLabel } from '../model/graph';
import { isFinished } from '../model/review';
import { RUN_STATE, RUN_STATUS_LABEL } from '../model/status';
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
  const { intent, navigate } = useNavigation();
  const active = workspace.active;
  const space = useWorkflows(active?.id);
  // Starting from an approved spec (ADR 0035): only while the module is on.
  const readySpecs = useReadySpecs(active?.id);
  const [startFrom, setStartFrom] = useState<StartFrom>(
    intent?.type === 'workflow-from-spec' ? { kind: 'spec', specId: intent.specId } : VIBE,
  );
  const startSpec =
    startFrom.kind === 'spec' ? readySpecs.find((s) => s.id === startFrom.specId) : undefined;

  const [selection, setSelection] = useState<Selection>(null);
  const [task, setTask] = useState('');
  // Where the next run works and what it may consult; the primary project alone until chosen.
  const [runScope, setRunScope] = useState<ScopeChoice | undefined>(undefined);
  // The person's standing choice about plans: kept between sessions, given to each new run.
  const [autoPlans, setAutoPlans] = useState(loadAutoPlans);
  const [connecting, setConnecting] = useState(false);
  const [creating, setCreating] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [savingTemplate, setSavingTemplate] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [inspected, setInspected] = useState<StoredExecution | null>(null);
  /** The diff is open: of all files (`''`) or of one. */
  const [reviewing, setReviewing] = useState<string | null>(null);
  const [repairing, setRepairing] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState(false);

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
        personalityId: agent.personalityId,
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

  // Where the dock reads the workspace's MCP connections, and how to get to the hub.
  const hubTools = {
    workspaceId: active.id,
    onOpenHub: () => {
      navigate('mcp');
    },
  };

  const create = (choice: NewWorkflowChoice) => {
    void space
      .create(choice.templateId, choice.mode, choice.name, choice.applyTools)
      .then((created) => {
        if (!created) return;
        setCreating(false);
        if (choice.task) setTask(choice.task);
        const notices: string[] = [];
        if (created.missing.length > 0) {
          notices.push(
            t('workflow.missingAgents', {
              roles: created.missing
                .map((role) => t(`workflow.role.${role}` as TranslationKey))
                .join(', '),
            }),
          );
        }
        // What the tools option did: said, so a step is never left to guess what it was given.
        const given = created.tools.filter((item) => item.granted).length;
        if (given > 0) notices.push(t('workflow.toolsGranted', { count: given }));
        const refused = created.tools.filter((item) => item.reason?.kind === 'refused');
        if (refused.length > 0) {
          notices.push(
            t('workflow.toolsSkipped', {
              list: refused.map((item) => item.connection ?? item.nodeId).join(', '),
            }),
          );
        }
        setNotice(notices.length > 0 ? notices.join(' ') : null);
      });
  };

  const afterImport = (report: ImportReportDto) => {
    const imported = report.items.find(
      (item) =>
        item.kind === 'workflow' && (item.result === 'created' || item.result === 'replaced'),
    );
    void space.reload(imported?.id ?? undefined).catch(() => {
      setNotice(t('portability.reloadFailed'));
    });
    setCreating(false);
  };

  if (space.workflows.length === 0 || creating) {
    return (
      <section className={styles.page}>
        <PageHeader
          title={t('workflow.newTitle')}
          {...(space.workflows.length > 0
            ? {
                back: {
                  label: t('workflow.title'),
                  onClick: () => {
                    setCreating(false);
                  },
                },
              }
            : {})}
          actions={
            <OverflowMenu
              label={t('workflow.more')}
              items={[
                {
                  label: t('portability.import'),
                  icon: 'download',
                  onSelect: () => {
                    setImporting(true);
                  },
                },
              ]}
            />
          }
        />
        <NewWorkflowPanel
          templates={space.templates}
          recommend={space.recommend}
          planTools={space.planTools}
          agentName={agentName}
          onDeleteTemplate={(id) => void space.removeTemplate(id)}
          error={space.error === undefined ? null : errorMessage(t, space.error)}
          onCreate={create}
          specs={readySpecs}
          startFrom={startFrom}
          onStartFrom={setStartFrom}
          onNewSpec={() => {
            navigate('specs', { type: 'new-spec' });
          }}
          {...(space.workflows.length > 0
            ? {
                onCancel: () => {
                  setCreating(false);
                },
              }
            : {})}
        />
        {importing && (
          <ImportDialog
            onImported={afterImport}
            onClose={() => {
              setImporting(false);
            }}
          />
        )}
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
  const canRun =
    editable &&
    (startFrom.kind === 'spec' ? startSpec !== undefined : task.trim() !== '') &&
    (space.validation?.valid ?? false);
  const inspectedAgent = inspected ? agents.find((a) => a.id === inspected.agentId) : undefined;

  const pending =
    shownRun?.interactions.filter((interaction) => interaction.status === 'pending') ?? [];
  const finished = shownRun !== undefined && isFinished(shownRun);
  const menu = [
    ...(editable
      ? [
          {
            label: t('workflow.saveTemplate'),
            icon: 'stack' as const,
            onSelect: () => {
              setSavingTemplate((value) => !value);
            },
          },
        ]
      : []),
    // Exporting only reads the workflow, so it stays available while a run locks the editing.
    ...(space.selected
      ? [
          {
            label: t('portability.export'),
            icon: 'upload' as const,
            onSelect: () => {
              setExporting(true);
            },
          },
        ]
      : []),
    ...(editable
      ? [
          {
            label: t('portability.import'),
            icon: 'download' as const,
            onSelect: () => {
              setImporting(true);
            },
          },
          {
            label: t('workflow.deleteWorkflow'),
            icon: 'trash' as const,
            danger: true,
            onSelect: () => {
              setConfirmingDelete(true);
            },
          },
        ]
      : []),
  ];
  const runMenu = [
    ...(runStatus === 'interrupted' && shownRun
      ? [
          {
            label: t('workflow.restart'),
            icon: 'refresh' as const,
            onSelect: () => {
              void space
                .cancel()
                .then(() =>
                  shownRun.scope
                    ? space.run(shownRun.task, shownRun.autoApprovePlans, shownRun.scope)
                    : space.run(shownRun.task, shownRun.autoApprovePlans),
                );
            },
          },
        ]
      : []),
    ...(shownRun && isActiveRun(shownRun)
      ? [
          {
            label: t('workflow.cancel'),
            icon: 'stop' as const,
            danger: true,
            onSelect: () => void space.cancel(),
          },
        ]
      : []),
  ];

  const runPicker = space.runs.length > 0 && (
    <select
      className={styles.runPicker}
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
  );

  return (
    <ReactFlowProvider>
      <section
        className={[styles.page, styles.pageFlush].join(' ')}
        data-mode={shownRun ? (finished ? 'review' : 'run') : 'edit'}
      >
        <h1 className={styles.srOnly}>{t('workflow.title')}</h1>
        {!shownRun ? (
          <header className={styles.bar}>
            <div className={styles.barLeft}>
              <span className={styles.switcher}>
                <select
                  className={styles.switcherSelect}
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
              </span>
              {workflow && (
                <Badge>
                  {t(`workflow.mode.${workflow.mode}` as TranslationKey)} ·{' '}
                  {t('workflow.version', { version: workflow.version })}
                </Badge>
              )}
              <Button
                variant="secondary"
                onClick={() => {
                  setCreating(true);
                }}
              >
                <Icon name="plus" size={16} />
                {t('workflow.new')}
              </Button>
            </div>
            <div className={styles.barRight}>
              {runPicker}
              {workflow?.mode === 'automatic' && editable && (
                <Button variant="secondary" onClick={space.customize}>
                  {t('workflow.customize')}
                </Button>
              )}
              {editable && (
                <Button
                  variant="secondary"
                  disabled={!space.dirty}
                  onClick={() => void space.save()}
                >
                  {t('common.save')}
                </Button>
              )}
              {menu.length > 0 && <OverflowMenu label={t('workflow.more')} items={menu} />}
            </div>
          </header>
        ) : (
          <header className={styles.bar}>
            <div className={styles.barLeft}>
              <Button
                variant="ghost"
                onClick={() => {
                  space.showRun(null);
                }}
              >
                <Icon name="arrowLeft" size={16} />
                {t('workflow.backToEditor')}
              </Button>
              {runPicker}
              <Badge>
                {t(`workflow.mode.${shownRun.workflow.mode}` as TranslationKey)} ·{' '}
                {t('workflow.version', { version: shownRun.workflow.version })}
              </Badge>
              {runStatus && (
                <span
                  role="status"
                  aria-label={t('workflow.runStatus', { status: t(RUN_STATUS_LABEL[runStatus]) })}
                >
                  <StateBadge state={RUN_STATE[runStatus]}>
                    {t(RUN_STATUS_LABEL[runStatus])}
                  </StateBadge>
                </span>
              )}
              {space.activeRun && (
                <AutoPlansToggle
                  checked={space.activeRun.autoApprovePlans}
                  onChange={(value) => {
                    void space.autoApprovePlans(value);
                  }}
                />
              )}
            </div>
            <div className={styles.barRight}>
              {runStatus === 'running' && (
                <Button variant="secondary" onClick={() => void space.pause()}>
                  {t('workflow.pause')}
                </Button>
              )}
              {(runStatus === 'paused' || runStatus === 'interrupted') && (
                <Button onClick={() => void space.resume()}>{t('workflow.resume')}</Button>
              )}
              {runMenu.length > 0 && <OverflowMenu label={t('workflow.runMore')} items={runMenu} />}
            </div>
          </header>
        )}

        {!shownRun && space.activeRun && (
          <Callout
            tone="warning"
            action={
              <Button
                variant="secondary"
                size="sm"
                onClick={() => {
                  space.showRun(space.activeRun?.id ?? null);
                }}
              >
                {t('workflow.openActiveRun')}
              </Button>
            }
          >
            {t('workflow.lockedByRun')}
          </Callout>
        )}

        {confirmingDelete && editable && (
          <div role="group" aria-label={t('workflow.deleteWorkflow')} className={styles.confirm}>
            <span>{t('workflow.deleteWorkflow')}?</span>
            <span className={styles.confirmActions}>
              <Button
                variant="ghost"
                onClick={() => {
                  setConfirmingDelete(false);
                }}
              >
                {t('common.cancel')}
              </Button>
              <Button
                variant="danger"
                onClick={() => {
                  setConfirmingDelete(false);
                  void space.remove();
                }}
              >
                {t('common.confirmDelete')}
              </Button>
            </span>
          </div>
        )}
        {editable && savingTemplate && workflow && (
          <SaveTemplate
            dirty={space.dirty}
            defaultName={workflow.name}
            onCancel={() => {
              setSavingTemplate(false);
            }}
            onSave={(name, description) => {
              void space.saveTemplate(name, description).then((saved) => {
                if (!saved) return;
                setSavingTemplate(false);
                setNotice(t('workflow.saveTemplate.saved', { name: saved.name }));
              });
            }}
          />
        )}

        {editable && (
          <div className={styles.runBar}>
            <StartChoice
              name="start-from-run"
              compact
              specs={readySpecs}
              value={startFrom}
              onChange={setStartFrom}
              onNewSpec={() => {
                navigate('specs', { type: 'new-spec' });
              }}
            />
            <input
              className={styles.taskInput}
              aria-label={t('workflow.task')}
              placeholder={t('workflow.task.placeholder')}
              value={task}
              onChange={(e) => {
                setTask(e.target.value);
              }}
            />
            {active.projects.length > 1 && (
              <ScopePicker
                projects={active.projects}
                choice={normalizeChoice(runScope, active)}
                onChange={setRunScope}
              />
            )}
            <AutoPlansToggle
              checked={autoPlans}
              onChange={(value) => {
                setAutoPlans(value);
                saveAutoPlans(value);
              }}
            />
            <Button
              disabled={!canRun}
              onClick={() => {
                const scope = toScopeRequest(runScope, active);
                if (startFrom.kind === 'spec') {
                  void space
                    .run(task.trim(), autoPlans, scope, startFrom.specId)
                    .then((started) => {
                      // A spec is built from on purpose, not by leaving the choice where it was.
                      if (started) setStartFrom(VIBE);
                    });
                } else if (scope) void space.run(task.trim(), autoPlans, scope);
                else void space.run(task.trim(), autoPlans);
              }}
            >
              <Icon name="play" size={14} />
              {t('workflow.run')}
            </Button>
          </div>
        )}

        {space.error !== undefined && (
          <p role="alert" className={styles.failure}>
            {errorMessage(t, space.error)}
          </p>
        )}
        {shownRun?.spec && (
          <SpecRunPanel
            link={shownRun.spec}
            runId={shownRun.id}
            stamp={shownRun.updatedAt}
            onOpenSpec={(specId) => {
              navigate('specs', { type: 'open-spec', specId });
            }}
          />
        )}
        {notice && (
          <p role="status" className={styles.warning}>
            {notice}
          </p>
        )}
        {workflow && (
          <>
            {finished && (
              <ReviewWorkspace
                key={shownRun.id}
                run={shownRun}
                ides={space.ides}
                busy={space.integrating}
                context={{
                  agentName,
                  personality: (agentId) => labels.get(agentId)?.personality ?? '',
                  executionOf: (executionId) => {
                    const stored = workspace.history.find(
                      (execution) => execution.id === executionId,
                    );
                    return stored
                      ? { runtime: runtimeName(stored.runtimeId), model: stored.modelId }
                      : undefined;
                  },
                }}
                onOpenExecution={openExecution}
                code={{
                  review: (file) => {
                    setReviewing(file ?? '');
                  },
                  openInIde: (ideId) => {
                    void space.openRunInIde(ideId);
                  },
                  apply: () => void space.integrate('apply'),
                  keep: () => void space.integrate('keep'),
                  discard: () => void space.integrate('discard'),
                }}
              />
            )}
            {finished && <h2 className={styles.graphTitle}>{t('workflow.graph')}</h2>}
            <div
              className={styles.workspace}
              data-drawer={selectedNode ? 'true' : 'false'}
              data-side={pending.length > 0 ? 'action' : shownRun ? 'run' : 'edit'}
            >
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
                overlay={
                  <>
                    {editable && (
                      <div className={styles.floatTools}>
                        <NodeTools
                          editable={editable}
                          agents={agents}
                          hasSelection={selection !== null}
                          canConnect={selection?.kind === 'node'}
                          connecting={connecting}
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
                        />
                        <span className={styles.toolRule} aria-hidden="true" />
                        <HistoryTools
                          editable={editable}
                          canUndo={space.canUndo}
                          canRedo={space.canRedo}
                          onUndo={space.undo}
                          onRedo={space.redo}
                          onResetLayout={() => {
                            space.edit(resetLayout);
                          }}
                        />
                      </div>
                    )}
                    <ZoomTools />
                  </>
                }
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
                    tools={{
                      workspaceId: active.id,
                      savedNodeIds: new Set(space.selected?.nodes.map((n) => n.id)),
                    }}
                    onClose={() => {
                      setSelection(null);
                    }}
                    save={{ dirty: space.dirty, onSave: () => void space.save() }}
                    valid={!invalid.has(selectedNode.id)}
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
                ) : pending.length > 0 ? (
                  <div className={styles.actionColumn}>
                    {pending.map((interaction) => (
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
                  </div>
                ) : shownRun ? (
                  <RunOverview
                    run={shownRun}
                    live={
                      !isFinished(shownRun) ? (
                        <LiveWorkspacePanel key={shownRun.id} run={shownRun} />
                      ) : null
                    }
                    tools={hubTools}
                    agentName={agentName}
                    onOpenExecution={openExecution}
                    ides={space.ides}
                    busy={space.integrating}
                    recovery={space.recovery}
                    onResume={() => void space.resume()}
                    code={{
                      review: (file) => {
                        setReviewing(file ?? '');
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
                    tools={hubTools}
                    validation={space.validation}
                    editable={editable}
                    onRename={(name) => {
                      space.edit((w) => ({ ...w, name }));
                    }}
                    repair={{
                      available: space.repairs.length > 0,
                      unsaved: space.dirty,
                      onReview: () => {
                        setRepairing(true);
                      },
                    }}
                  />
                )}
              </aside>
            </div>
          </>
        )}

        {repairing && workflow && space.repairs.length > 0 && (
          <RouteRepairDialog
            workflow={workflow}
            proposals={space.repairs}
            busy={false}
            onCancel={() => {
              setRepairing(false);
            }}
            onApply={(choices) => {
              void space.repairRoutes(choices).then(() => {
                setRepairing(false);
              });
            }}
          />
        )}

        {reviewing !== null && shownRun && (
          <ChangesModal
            run={shownRun}
            initialFile={reviewing === '' ? null : reviewing}
            onClose={() => {
              setReviewing(null);
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
        {exporting && space.selected && (
          <ExportDialog
            targets={[{ kind: 'workflow', id: space.selected.id }]}
            title={t('portability.export.title', { name: space.selected.name })}
            onClose={() => {
              setExporting(false);
            }}
          />
        )}
        {importing && (
          <ImportDialog
            onImported={afterImport}
            onClose={() => {
              setImporting(false);
            }}
          />
        )}
      </section>
    </ReactFlowProvider>
  );
}
