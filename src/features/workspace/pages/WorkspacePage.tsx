import { useState } from 'react';
import { useNavigation } from '@/app/NavigationContext';
import { Icon } from '@/components/ui/Icon';
import { Button } from '@/components/ui/Button';
import { Callout } from '@/components/ui/Callout';
import { PageHeader } from '@/components/ui/PageHeader';
import { SegmentedControl } from '@/components/ui/SegmentedControl';
import { useCatalog } from '@/features/agents/hooks/useCatalog';
import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import { AddAgentPanel } from '../components/AddAgentPanel';
import { AgentCard } from '../components/AgentCard';
import { AgentDetailsPanel } from '../components/AgentDetailsPanel';
import { EditAgentPanel } from '../components/EditAgentPanel';
import { OnboardingPanel } from '../components/OnboardingPanel';
import { WorkspaceProjectsPanel } from '../components/WorkspaceProjectsPanel';
import { WorkspaceGrid } from '../components/WorkspaceGrid';
import { WorkspaceUsageChip } from '../components/WorkspaceUsageChip';
import { useWorkspace } from '../hooks/WorkspaceProvider';
import { agentStatus } from '../model/agentStatus';
import { MAX_WORKSPACE_AGENTS, canAddAgent } from '../model/grid';
import type { ScopeChoice } from '../model/scope';
import { runKey, type ConversationsState } from '../model/agentRuns';
import styles from '../components/Workspace.module.css';

const VIEW_KEY = 'atlas.workspace.view';
type View = 'list' | 'grid';

function readView(): View {
  try {
    return localStorage.getItem(VIEW_KEY) === 'grid' ? 'grid' : 'list';
  } catch {
    return 'list';
  }
}

/** The live process of the agent's latest run, if it has one. */
function processOf(conversations: ConversationsState, key: string) {
  const run = conversations.runs[key];
  return run ? conversations.processes[run.executionId] : undefined;
}

/**
 * The active workspace: its project, its agents laid out on the grid, and a chat per agent. All
 * of it belongs to the workspace; switching workspaces shows another set, while agents that are
 * running elsewhere keep running.
 */
export function WorkspacePage() {
  const t = useT();
  const {
    catalog,
    runtimes,
    refreshRuntimes,
    editAgent,
    removeAgent: deleteAgentEverywhere,
  } = useCatalog();
  const workspace = useWorkspace();
  const { navigate } = useNavigation();
  const [adding, setAdding] = useState(false);
  const [managingProjects, setManagingProjects] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [detailsId, setDetailsId] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [view, setView] = useState<View>(readView);
  // Agents folded down to their header in the expandable list; everything starts open.
  const [folded, setFolded] = useState<ReadonlySet<string>>(new Set());

  if (catalog.status === 'loading' || workspace.state.status === 'loading') {
    return <p className={styles.muted}>{t('common.loadingCore')}</p>;
  }
  if (catalog.status === 'error' || workspace.state.status === 'error') {
    const error =
      catalog.status === 'error'
        ? catalog.error
        : workspace.state.status === 'error'
          ? workspace.state.error
          : undefined;
    return (
      <p role="alert" className={styles.notice}>
        {t('common.coreUnavailable', { message: errorMessage(t, error) })}
      </p>
    );
  }
  const active = workspace.active;
  if (!active) return <OnboardingPanel />;
  const current = active;

  const { agents, personalities } = catalog;
  const placedIds = new Set(active.layout.agentPlacements.map((p) => p.agentId));
  const unplaced = agents.filter((agent) => !placedIds.has(agent.id));
  const editing = agents.find((agent) => agent.id === editingId);
  const details = agents.find((agent) => agent.id === detailsId);
  const runtimeOf = (runtimeId: string) =>
    runtimes.status === 'ready'
      ? runtimes.runtimes.find((r) => r.runtime.id === runtimeId)
      : undefined;
  const { conversations } = workspace;

  function chooseView(next: View) {
    setView(next);
    try {
      localStorage.setItem(VIEW_KEY, next);
    } catch {
      // The choice simply is not remembered.
    }
  }

  function toggleFolded(agentId: string) {
    setFolded((current) => {
      const next = new Set(current);
      if (!next.delete(agentId)) next.add(agentId);
      return next;
    });
  }

  const placedAgentIds = active.layout.agentPlacements
    .slice()
    .sort((a, b) => a.position.row - b.position.row || a.position.column - b.position.column)
    .map((p) => p.agentId)
    .filter((id) => agents.some((a) => a.id === id));
  const allFolded = placedAgentIds.length > 0 && placedAgentIds.every((id) => folded.has(id));

  function openAdd() {
    if (!canAddAgent(current)) {
      setNotice(t('workspace.capacity', { max: MAX_WORKSPACE_AGENTS }));
      return;
    }
    setNotice(null);
    setEditingId(null);
    setAdding(true);
  }

  function addExisting(agentId: string) {
    void workspace.addAgent(agentId).then((result) => {
      if (result.ok) {
        setNotice(null);
        setAdding(false);
      } else {
        setNotice(
          result.capacity
            ? t('workspace.capacity', { max: MAX_WORKSPACE_AGENTS })
            : errorMessage(t, result.error),
        );
      }
    });
  }

  /** The scope picker of an agent, for a workspace that has more than one project to choose from. */
  function scopeFor(agentId: string) {
    const choice = workspace.scopeOf(agentId);
    if (current.projects.length < 2 || !choice) return {};
    return {
      scope: {
        projects: current.projects,
        choice,
        onChange: (next: ScopeChoice) => {
          workspace.setScopeOf(agentId, next);
        },
      },
    };
  }

  function renderAgent(agentId: string, variant: View) {
    const agent = agents.find((a) => a.id === agentId);
    if (!agent) return null;
    const key = runKey(current.id, agentId);
    return (
      <AgentCard
        workspaceId={current.id}
        agent={agent}
        personality={personalities.find((p) => p.id === agent.personalityId)}
        runtime={runtimeOf(agent.runtimeId)}
        workspaceName={current.name}
        messages={conversations.messages[key] ?? []}
        history={workspace.history.filter(
          (e) => e.workspaceId === current.id && e.agentId === agentId,
        )}
        focusTab={
          workspace.focus?.workspaceId === current.id && workspace.focus.agentId === agentId
            ? workspace.focus.tab
            : undefined
        }
        onFocusHandled={workspace.clearFocus}
        run={conversations.runs[key]}
        process={processOf(conversations, key)}
        terminals={workspace.terminals}
        liveText={conversations.streams[key]?.text}
        sendError={conversations.sendErrors[key]}
        onSend={(content) => {
          void workspace.sendToAgent(agentId, content);
        }}
        {...scopeFor(agentId)}
        onOpenDetails={() => {
          setDetailsId(agentId);
        }}
        onOpenWorkflow={(link) => {
          navigate('workflow', {
            type: 'open-workflow',
            workflowId: link.workflowId,
            executionId: link.workflowExecutionId,
          });
        }}
        onEdit={() => {
          setAdding(false);
          setEditingId(agentId);
        }}
        onRemove={() => {
          void workspace.removeAgent(agentId);
        }}
        variant={variant}
        expanded={!folded.has(agentId)}
        onToggle={() => {
          toggleFolded(agentId);
        }}
      />
    );
  }

  if (managingProjects) {
    return (
      <WorkspaceProjectsPanel
        workspace={active}
        onWorkspaceChanged={workspace.replaceWorkspace}
        onClose={() => {
          setManagingProjects(false);
        }}
      />
    );
  }

  return (
    <section className={styles.page} data-view={view}>
      <PageHeader
        title={active.name}
        meta={<WorkspaceUsageChip workspaceId={active.id} usageVersion={workspace.usageVersion} />}
        actions={
          <>
            <Button
              variant="secondary"
              aria-label={t('projects.openLabel', { name: active.name })}
              onClick={() => {
                setManagingProjects(true);
              }}
            >
              {t('projects.open', { count: active.projects.length })}
            </Button>
            <Button
              variant="secondary"
              onClick={() => {
                navigate('mcp');
              }}
            >
              {t('integrations.open')}
            </Button>
            <Button onClick={openAdd}>
              <Icon name="plus" size={16} />
              {t('workspace.addAgent')}
            </Button>
          </>
        }
      />

      {notice && (
        <Callout tone="danger" role="alert">
          {notice}
        </Callout>
      )}
      {editing && (
        <EditAgentPanel
          agent={editing}
          personalities={personalities}
          runtimes={runtimes}
          onRefreshRuntimes={refreshRuntimes}
          onSave={(input) => editAgent(editing.id, input)}
          onDelete={async () => {
            await deleteAgentEverywhere(editing.id);
            setEditingId(null);
          }}
          onClose={() => {
            setEditingId(null);
          }}
        />
      )}
      {adding && (
        <AddAgentPanel
          available={unplaced}
          onAdd={addExisting}
          onCreateNew={() => {
            navigate('agents', { type: 'create-agent' });
          }}
          onClose={() => {
            setAdding(false);
          }}
        />
      )}
      {active.layout.agentPlacements.length === 0 && !adding && (
        <p className={styles.empty}>{t('workspace.noAgents')}</p>
      )}

      {placedAgentIds.length > 0 && (
        <div className={styles.viewBar}>
          <SegmentedControl
            label={t('workspace.view.label')}
            value={view}
            onChange={chooseView}
            segments={[
              { value: 'list', label: t('workspace.view.list'), icon: 'viewList' },
              { value: 'grid', label: t('workspace.view.grid'), icon: 'viewGrid' },
            ]}
          />
          {view === 'list' && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => {
                setFolded(allFolded ? new Set() : new Set(placedAgentIds));
              }}
            >
              <Icon name={allFolded ? 'unfold' : 'collapseAll'} size={16} />
              {allFolded ? t('workspace.expandAll') : t('workspace.collapseAll')}
            </Button>
          )}
        </div>
      )}

      {view === 'list' ? (
        <ul className={styles.agentList}>
          {placedAgentIds.map((agentId) => (
            <li key={agentId}>{renderAgent(agentId, 'list')}</li>
          ))}
        </ul>
      ) : (
        <WorkspaceGrid
          workspace={active}
          renderAgent={(agentId) => renderAgent(agentId, 'grid')}
          onAdd={openAdd}
        />
      )}

      {details && (
        <AgentDetailsPanel
          workspaceId={active.id}
          agent={details}
          personality={personalities.find((p) => p.id === details.personalityId)}
          runtime={runtimeOf(details.runtimeId)}
          run={conversations.runs[runKey(active.id, details.id)]}
          status={agentStatus(
            conversations.runs[runKey(active.id, details.id)],
            runtimeOf(details.runtimeId),
            processOf(conversations, runKey(active.id, details.id)),
          )}
          usageVersion={workspace.usageVersion}
          latest={workspace.history.find(
            (e) => e.workspaceId === active.id && e.agentId === details.id,
          )}
          onOpenExecution={() => {
            setDetailsId(null);
            setFolded((current) => {
              const next = new Set(current);
              next.delete(details.id);
              return next;
            });
            void workspace.focusAgent({
              workspaceId: active.id,
              agentId: details.id,
              tab: 'executions',
            });
          }}
          onEdit={() => {
            setDetailsId(null);
            setAdding(false);
            setEditingId(details.id);
          }}
          onClose={() => {
            setDetailsId(null);
          }}
        />
      )}
    </section>
  );
}
