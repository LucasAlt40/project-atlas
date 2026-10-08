import { useState } from 'react';
import { Modal } from '@/components/ui/Modal';
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
  const { t } = useI18n();
  const [tab, setTab] = useState<Tab>('details');
  const mine = messages.filter((message) => message.executionId === execution.id);

  return (
    <Modal label={t('inspector.title', { id: shortId(execution.id) })} onClose={onClose}>
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
      <h2>{t('inspector.title', { id: shortId(execution.id) })}</h2>
      <div className={styles.tabs} role="tablist" aria-label={t('inspector.tabs')}>
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
          </button>
        ))}
      </div>
      <div role="tabpanel">
        {tab === 'details' && (
          <ExecutionDetails
            facts={factsFromStored(execution)}
            context={context}
            {...(asked ? { asked } : {})}
          />
        )}
        {tab === 'context' &&
          (execution.manifest ? (
            <ContextPlatformPanel execution={execution} />
          ) : (
            <p className={styles.note}>{t('inspector.context.none')}</p>
          ))}
        {tab === 'activity' && <AgentActivity run={runFromStored(execution)} />}
        {tab === 'chat' &&
          (mine.length > 0 ? (
            <AgentMessageList messages={mine} />
          ) : (
            <p className={styles.note}>{t('inspector.noMessages')}</p>
          ))}
        {tab === 'terminal' && <p className={styles.note}>{t('inspector.terminalNotSaved')}</p>}
      </div>
    </Modal>
  );
}
