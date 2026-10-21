import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/Button';
import type { IconName } from '@/components/ui/Icon';
import { Tabs } from '@/components/ui/Tabs';
import type { Agent, Personality, RuntimeStatus } from '@/features/agents/types';
import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import type { WorkflowLinkDto } from '@/lib/tauri/commands';
import type { AgentRun, ProcessState } from '../model/agentRuns';
import { agentStatus } from '../model/agentStatus';
import type { TerminalHub } from '../model/terminalHub';
import { factsFromRun, factsFromStored } from '../model/inspection';
import type { Message, StoredExecution } from '../types';
import { AgentActivity } from './AgentActivity';
import styles from './AgentCard.module.css';
import { AgentComposer, type AgentScope } from './AgentComposer';
import { AgentHeader } from './AgentHeader';
import { AgentMessageList } from './AgentMessageList';
import { ExecutionDetails, type ExecutionContext } from './ExecutionDetails';
import { ExecutionHistory } from './ExecutionHistory';
import { ExecutionView } from './ExecutionView';
import { RunControls } from './RunControls';
import { TerminalPanel } from './TerminalPanel';
import inspectorStyles from './Inspector.module.css';

type Tab = 'chat' | 'activity' | 'terminal' | 'details' | 'executions';
const TABS: readonly Tab[] = ['chat', 'activity', 'terminal', 'details', 'executions'];
const TAB_ICON: Record<Tab, IconName> = {
  chat: 'chat',
  activity: 'history',
  terminal: 'terminal',
  details: 'info',
  executions: 'play',
};

interface Props {
  workspaceId: string;
  agent: Agent;
  personality: Personality | undefined;
  runtime: RuntimeStatus | undefined;
  workspaceName: string;
  messages: Message[];
  /** This agent's ended executions in the workspace, newest first. */
  history: StoredExecution[];
  /** A request to show a view (from the global list of active executions). */
  focusTab: Tab | undefined;
  onFocusHandled: () => void;
  run: AgentRun | undefined;
  /** The live process of the run, when it has one. */
  process: ProcessState | undefined;
  /** Where terminal output is kept, for every execution. */
  terminals: TerminalHub;
  liveText: string | undefined;
  /** Why the last send was rejected, if it was. */
  sendError: unknown;
  onSend: (content: string) => void;
  /** Where the agent works and what else it may consult. Absent: one project, nothing to choose. */
  scope?: AgentScope;
  onOpenDetails: () => void;
  /** Takes the user to the workflow run an inspected execution belongs to. */
  onOpenWorkflow?: (link: WorkflowLinkDto) => void;
  onEdit: () => void;
  onRemove: () => void;
  /**
   * `list` is the expandable list: the card takes the height its content needs and can fold
   * down to its header. `grid` fills its cell.
   */
  variant?: 'grid' | 'list';
  expanded?: boolean;
  onToggle?: () => void;
}

/**
 * One workspace item. It knows nothing about positions or other agents, so it can later be
 * dragged, resized, pinned or maximized by whatever lays it out. Three views of the same
 * agent: the conversation (what it says), the activity (what happened) and the terminal (what
 * its process does).
 */
export function AgentCard({
  workspaceId,
  agent,
  personality,
  runtime,
  workspaceName,
  messages,
  history,
  focusTab,
  onFocusHandled,
  run,
  process,
  terminals,
  liveText,
  sendError,
  onSend,
  scope,
  onOpenDetails,
  onOpenWorkflow,
  onEdit,
  onRemove,
  variant = 'grid',
  expanded = true,
  onToggle,
}: Props) {
  const t = useT();
  const [tab, setTab] = useState<Tab>('chat');
  // The execution shown beside the list in the Executions tab (the latest by default).
  const [selectedId, setSelectedId] = useState<string | null>(null);
  // A request from outside is applied once, then acknowledged so it does not repeat.
  const [shownFocus, setShownFocus] = useState<Tab | undefined>(undefined);
  if (focusTab !== shownFocus) {
    setShownFocus(focusTab);
    if (focusTab) setTab(focusTab);
  }
  useEffect(() => {
    if (focusTab) onFocusHandled();
  }, [focusTab, onFocusHandled]);
  const status = agentStatus(run, runtime, process);
  const capabilities = runtime?.runtime.capabilities;
  const runtimeName = runtime?.runtime.name ?? agent.runtimeId;
  const context: ExecutionContext = {
    workspaceName,
    agentName: agent.name,
    personalityName: personality?.name ?? agent.personalityId,
    runtimeName,
    capabilities,
    worktreeIsolation: agent.worktreeIsolation,
  };
  const runtimeNameOf = (runtimeId: string) =>
    runtimeId === agent.runtimeId ? runtimeName : runtimeId;
  // The run on screen if there is one, else the latest ended execution.
  const stored = run ? history.find((e) => e.id === run.executionId) : history[0];
  const lastTask = [...messages]
    .reverse()
    .find((m) => m.role === 'user' && m.executionId === run?.executionId);
  const facts = stored
    ? factsFromStored(stored)
    : run
      ? factsFromRun(run, process, agent, lastTask?.content ?? null)
      : null;
  const idPrefix = `agent-${agent.id}`;
  const selected = history.find((e) => e.id === selectedId) ?? history[0];
  const ended = run && run.status !== 'running' && stored ? stored : null;

  function chooseTab(next: Tab) {
    setTab(next);
    // The agent's details are a drawer over the screen; the tab keeps the facts of its run.
    if (next === 'details') onOpenDetails();
  }

  return (
    <article
      className={styles.card}
      aria-label={agent.name}
      data-status={status}
      data-variant={variant}
      data-expanded={expanded}
    >
      <AgentHeader
        name={agent.name}
        personalityName={personality?.name ?? agent.personalityId}
        runtimeName={runtimeName}
        providerName={runtime?.runtime.provider.name ?? null}
        modelId={agent.modelId}
        gitIsolation={agent.worktreeIsolation}
        status={status}
        variant={variant}
        {...(variant === 'list' && onToggle
          ? {
              expanded,
              onToggle,
              onOpenTerminal: () => {
                setTab('terminal');
                if (!expanded) onToggle();
              },
            }
          : {})}
        onOpenDetails={onOpenDetails}
        onEdit={onEdit}
        onRemove={onRemove}
      />
      {(variant === 'grid' || expanded) && (
        <div className={styles.body}>
          <div className={styles.tabStrip}>
            <Tabs
              label={t('agent.tabs')}
              idPrefix={idPrefix}
              active={tab}
              onChange={chooseTab}
              items={TABS.map((name) => ({
                id: name,
                label: t(`agent.tab.${name}`),
                ...(variant === 'list' ? { icon: TAB_ICON[name] } : {}),
                ...(name === 'executions' && history.length > 0 ? { count: history.length } : {}),
              }))}
              {...(ended && tab !== 'executions'
                ? {
                    trailing: (
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => {
                          setSelectedId(ended.id);
                          setTab('executions');
                        }}
                      >
                        {t('agent.viewExecution')}
                      </Button>
                    ),
                  }
                : {})}
            />
          </div>
          {tab !== 'terminal' &&
            run?.status === 'running' &&
            process &&
            process.status !== 'exited' && (
              <RunControls
                workspaceId={workspaceId}
                agentId={agent.id}
                run={run}
                process={process}
                capabilities={capabilities}
                onOpenTerminal={() => {
                  setTab('terminal');
                }}
              />
            )}
          <div
            id={`${idPrefix}-panel-${tab}`}
            role="tabpanel"
            aria-labelledby={`${idPrefix}-tab-${tab}`}
            className={styles.panel}
            data-tab={tab}
          >
            {tab === 'chat' && (
              <AgentMessageList messages={messages} liveText={liveText} agentName={agent.name} />
            )}
            {tab === 'activity' && <AgentActivity run={run} />}
            {tab === 'terminal' && (
              <TerminalPanel
                workspaceId={workspaceId}
                agentId={agent.id}
                agentName={agent.name}
                runtimeName={runtimeName}
                capabilities={capabilities}
                run={run}
                process={process}
                hub={terminals}
              />
            )}
            {tab === 'details' &&
              (facts ? (
                <ExecutionDetails facts={facts} context={context} />
              ) : (
                <p className={styles.empty}>{t('inspector.noExecution')}</p>
              ))}
            {tab === 'executions' &&
              (history.length === 0 ? (
                <p className={styles.empty}>{t('executions.empty')}</p>
              ) : (
                <div className={inspectorStyles.masterDetail}>
                  <nav className={inspectorStyles.master} aria-label={t('executions.list')}>
                    <ExecutionHistory
                      executions={history}
                      runtimeName={runtimeNameOf}
                      selectedId={selected?.id ?? null}
                      onOpen={(execution) => {
                        setSelectedId(execution.id);
                      }}
                    />
                  </nav>
                  <div className={inspectorStyles.detail}>
                    {selected && (
                      <ExecutionView
                        key={selected.id}
                        execution={selected}
                        messages={messages}
                        context={context}
                        {...(onOpenWorkflow ? { onOpenWorkflow } : {})}
                      />
                    )}
                  </div>
                </div>
              ))}
          </div>
          {sendError !== undefined && (
            <p role="alert" className={styles.error}>
              {errorMessage(t, sendError)}
            </p>
          )}
          <AgentComposer
            workspaceId={workspaceId}
            agentId={agent.id}
            agentName={agent.name}
            disabled={run?.status === 'running'}
            onSend={onSend}
            compact={variant === 'grid'}
            {...(scope ? { scope } : {})}
          />
        </div>
      )}
    </article>
  );
}
