import { useState } from 'react';
import { useNavigation } from '@/app/NavigationContext';
import { Button } from '@/components/ui/Button';
import { useCatalog } from '@/features/agents/hooks/useCatalog';
import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import { AddAgentPanel } from '../components/AddAgentPanel';
import { AgentCard } from '../components/AgentCard';
import { AgentDetailsPanel } from '../components/AgentDetailsPanel';
import { EditAgentPanel } from '../components/EditAgentPanel';
import { OnboardingPanel } from '../components/OnboardingPanel';
import { ProjectContextBar } from '../components/ProjectContextBar';
import { WorkspaceGrid } from '../components/WorkspaceGrid';
import { WorkspaceUsageChip } from '../components/WorkspaceUsageChip';
import { useWorkspace } from '../hooks/WorkspaceProvider';
import { agentStatus } from '../model/agentStatus';
import { MAX_WORKSPACE_AGENTS, canAddAgent } from '../model/grid';
import { runKey, type ConversationsState } from '../model/agentRuns';
import styles from '../components/Workspace.module.css';

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
  const [editingId, setEditingId] = useState<string | null>(null);
  const [detailsId, setDetailsId] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

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

  return (
    <section className={styles.page}>
      <header className={styles.header}>
        <h1 className={styles.title}>{active.name}</h1>
        <div className={styles.headerRight}>
          <WorkspaceUsageChip workspaceId={active.id} usageVersion={workspace.usageVersion} />
          <Button onClick={openAdd}>{t('workspace.addAgent')}</Button>
        </div>
      </header>
      <ProjectContextBar workspace={active} />

      {notice && (
        <p role="alert" className={styles.notice}>
          {notice}
        </p>
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
        <p className={styles.muted}>{t('workspace.noAgents')}</p>
      )}

      <WorkspaceGrid
        workspace={active}
        renderAgent={(agentId) => {
          const agent = agents.find((a) => a.id === agentId);
          if (!agent) return null;
          const key = runKey(active.id, agentId);
          return (
            <AgentCard
              workspaceId={active.id}
              agent={agent}
              personality={personalities.find((p) => p.id === agent.personalityId)}
              runtime={runtimeOf(agent.runtimeId)}
              workspaceName={active.name}
              messages={conversations.messages[key] ?? []}
              history={workspace.history.filter(
                (e) => e.workspaceId === active.id && e.agentId === agentId,
              )}
              focusTab={
                workspace.focus?.workspaceId === active.id && workspace.focus.agentId === agentId
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
            />
          );
        }}
      />

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
          onClose={() => {
            setDetailsId(null);
          }}
        />
      )}
    </section>
  );
}
