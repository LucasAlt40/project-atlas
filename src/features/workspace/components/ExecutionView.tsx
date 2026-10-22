import { useState } from 'react';
import { StateBadge } from '@/components/ui/status';
import { Tabs } from '@/components/ui/Tabs';
import { formatCost, formatDuration } from '@/features/usage/model/format';
import { useI18n } from '@/i18n/I18nProvider';
import type { PendingInteractionDto, WorkflowLinkDto } from '@/lib/tauri/commands';
import { factsFromStored, runFromStored, shortId } from '../model/inspection';
import type { Message, StoredExecution } from '../types';
import { AgentActivity } from './AgentActivity';
import { AgentMessageList } from './AgentMessageList';
import { ContextPlatformPanel } from './ContextPlatformPanel';
import { ExecutionDetails, type ExecutionContext } from './ExecutionDetails';
import { AGENT_STATE } from './StatusBadge';
import styles from './Inspector.module.css';

type Tab = 'details' | 'context' | 'activity' | 'chat' | 'terminal';
const TABS: readonly Tab[] = ['details', 'context', 'activity', 'chat', 'terminal'];

export interface ExecutionViewProps {
  execution: StoredExecution;
  /** The agent's conversation in the workspace: the view shows this execution's part. */
  messages: Message[];
  context: ExecutionContext;
  /** Takes the user to the workflow run this execution is a step of (when it is one). */
  onOpenWorkflow?: (link: WorkflowLinkDto) => void;
  /** The question this execution asked, as its workflow run kept it. */
  asked?: PendingInteractionDto;
}

/**
 * One execution that has ended, in detail: its task and state, then facts, the context it was
 * given, its timeline and its conversation. Its terminal was live only, so that tab says so.
 */
export function ExecutionView({
  execution,
  messages,
  context,
  onOpenWorkflow,
  asked,
}: ExecutionViewProps) {
  const { t, language } = useI18n();
  const [tab, setTab] = useState<Tab>('details');
  const mine = messages.filter((message) => message.executionId === execution.id);
  const facts = factsFromStored(execution);
  const run = runFromStored(execution);
  const duration = facts.endedAt === null ? null : facts.endedAt - facts.startedAt;
  const cost =
    facts.usage?.cost === null || facts.usage?.cost === undefined
      ? null
      : formatCost(facts.usage.cost, facts.usage.currency, language);
  const state =
    facts.status === 'running' ? 'running' : (AGENT_STATE[facts.status] ?? ('pending' as const));
  const idPrefix = `execution-${execution.id}`;

  return (
    <section
      className={styles.view}
      aria-label={t('inspector.title', { id: shortId(execution.id) })}
    >
      <header className={styles.viewHead}>
        {execution.workflow && (
          <nav className={styles.breadcrumb} aria-label={t('inspector.breadcrumb')}>
            <ol>
              <li>
                {onOpenWorkflow ? (
                  <button
                    type="button"
                    className={styles.crumbLink}
                    onClick={() => {
                      if (execution.workflow) onOpenWorkflow(execution.workflow);
                    }}
                  >
                    {t('inspector.breadcrumb.workflow')}
                  </button>
                ) : (
                  t('inspector.breadcrumb.workflow')
                )}
              </li>
              <li>{execution.workflow.workflowName}</li>
              <li>
                {execution.workflow.nodeLabel}
                {execution.workflow.attempt > 1 &&
                  ` (${t('inspector.breadcrumb.attempt', { attempt: execution.workflow.attempt })})`}
              </li>
              <li aria-current="page">
                {t('inspector.breadcrumb.execution', { id: shortId(execution.id) })}
              </li>
            </ol>
          </nav>
        )}
        <div className={styles.titleRow}>
          <h3 className={styles.viewTitle}>{facts.task ?? t('inspector.title', { id: '' })}</h3>
          <StateBadge state={state}>{t(`agent.status.${facts.status}`)}</StateBadge>
        </div>
        <p className={styles.viewMeta}>
          <span>{t('inspector.title', { id: shortId(execution.id) })}</span>
          {duration !== null && <span>{formatDuration(duration)}</span>}
          {cost && <span>{cost}</span>}
        </p>
      </header>
      <Tabs
        label={t('inspector.tabs')}
        idPrefix={idPrefix}
        active={tab}
        onChange={setTab}
        items={TABS.map((name) => ({
          id: name,
          label: t(`inspector.tab.${name}`),
          ...(name === 'activity' && run.activity.length > 0 ? { count: run.activity.length } : {}),
        }))}
      />
      <div
        role="tabpanel"
        id={`${idPrefix}-panel-${tab}`}
        aria-labelledby={`${idPrefix}-tab-${tab}`}
        className={styles.viewBody}
      >
        {tab === 'details' && (
          <ExecutionDetails
            facts={facts}
            context={context}
            showTask={false}
            {...(asked ? { asked } : {})}
          />
        )}
        {tab === 'context' &&
          (execution.manifest ? (
            <ContextPlatformPanel execution={execution} />
          ) : (
            <p className={styles.note}>{t('inspector.context.none')}</p>
          ))}
        {tab === 'activity' && <AgentActivity run={run} />}
        {tab === 'chat' &&
          (mine.length > 0 ? (
            <AgentMessageList messages={mine} />
          ) : (
            <p className={styles.note}>{t('inspector.noMessages')}</p>
          ))}
        {tab === 'terminal' && <p className={styles.note}>{t('inspector.terminalNotSaved')}</p>}
      </div>
    </section>
  );
}
