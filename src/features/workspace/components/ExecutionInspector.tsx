import { useState } from 'react';
import { Badge } from '@/components/ui/Badge';
import { Modal } from '@/components/ui/Modal';
import { formatCost, formatDuration } from '@/features/usage/model/format';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type { PendingInteractionDto, WorkflowLinkDto } from '@/lib/tauri/commands';
import { factsFromStored, runFromStored, shortId } from '../model/inspection';
import type { Message, StoredExecution } from '../types';
import { AgentActivity } from './AgentActivity';
import { AgentMessageList } from './AgentMessageList';
import { ContextPlatformPanel } from './ContextPlatformPanel';
import { ExecutionDetails, type ExecutionContext } from './ExecutionDetails';
import styles from './Inspector.module.css';
import terminalStyles from './Terminal.module.css';

type Tab = 'details' | 'context' | 'activity' | 'chat' | 'terminal';
const TABS: readonly Tab[] = ['details', 'context', 'activity', 'chat', 'terminal'];

interface Props {
  execution: StoredExecution;
  /** The agent's conversation in the workspace: the inspector shows this execution's part. */
  messages: Message[];
  context: ExecutionContext;
  onClose: () => void;
  /** Takes the user to the workflow run this execution is a step of (when it is one). */
  onOpenWorkflow?: (link: WorkflowLinkDto) => void;
  /** The question this execution asked, as its workflow run kept it. */
  asked?: PendingInteractionDto;
}

/**
 * One execution that has ended, in detail: facts, timeline and conversation. Its terminal was
 * live only, so that tab says so instead of showing an empty screen.
 */
export function ExecutionInspector({
  execution,
  messages,
  context,
  onClose,
  onOpenWorkflow,
  asked,
}: Props) {
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
  const statusTone =
    facts.status === 'completed' ? 'success' : facts.status === 'failed' ? 'danger' : 'warning';

  return (
    <Modal
      label={t('inspector.title', { id: shortId(execution.id) })}
      onClose={onClose}
      size="inspector"
    >
      <header className={styles.inspectorHead}>
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
          <Badge mono>{t('inspector.badge', { id: shortId(execution.id) })}</Badge>
          <div className={styles.titleText}>
            <h2>{t('inspector.title', { id: shortId(execution.id) })}</h2>
            {facts.task && <p className={styles.subtitle}>{facts.task}</p>}
          </div>
          <span className={styles.titleChips}>
            <Badge dot tone={statusTone}>
              {t(`agent.status.${facts.status}`)}
            </Badge>
            {duration !== null && (
              <span className={styles.chip}>
                {t('inspector.duration')}: {formatDuration(duration)}
              </span>
            )}
            {cost && (
              <span className={styles.chip}>
                {t('details.cost')}: {cost}
              </span>
            )}
          </span>
        </div>
      </header>
      <div className={terminalStyles.tabs} role="tablist" aria-label={t('inspector.tabs')}>
        {TABS.map((name) => (
          <button
            key={name}
            type="button"
            role="tab"
            className={terminalStyles.tab}
            aria-selected={tab === name}
            onClick={() => {
              setTab(name);
            }}
          >
            {t(`inspector.tab.${name}` as TranslationKey)}
            {name === 'activity' && run.activity.length > 0 && (
              <span aria-hidden="true"> {run.activity.length}</span>
            )}
          </button>
        ))}
      </div>
      <div className={styles.inspectorBody} data-aside={tab !== 'activity'}>
        <div role="tabpanel" className={styles.inspectorMain}>
          {tab === 'details' && (
            <>
              <h3 className={styles.sectionTitle}>{t('inspector.parameters')}</h3>
              <ExecutionDetails
                facts={facts}
                context={context}
                showTask={false}
                {...(asked ? { asked } : {})}
              />
            </>
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
        {tab !== 'activity' && (
          <aside className={styles.inspectorAside} aria-label={t('inspector.timeline')}>
            <div className={styles.asideHead}>
              <h3>{t('inspector.timeline')}</h3>
              <span className={styles.chip}>
                {t('inspector.events', { count: run.activity.length })}
              </span>
            </div>
            <AgentActivity run={run} />
          </aside>
        )}
      </div>
    </Modal>
  );
}
