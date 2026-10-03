import { useState } from 'react';
import { AgentForm } from '@/features/agents/components/AgentForm';
import type { RuntimesState } from '@/features/agents/hooks/useCatalog';
import type { Agent, CreateAgentInput, Personality } from '@/features/agents/types';
import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import styles from './Workspace.module.css';

interface Props {
  agent: Agent;
  personalities: Personality[];
  runtimes: RuntimesState;
  onRefreshRuntimes: () => void;
  onSave: (input: CreateAgentInput) => Promise<Agent>;
  /** Deletes the agent for good (its conversations and workspace slots too). */
  onDelete: () => Promise<void>;
  onClose: () => void;
}

/** Edits an agent where it sits; deleting is a separate, confirmed step. */
export function EditAgentPanel({
  agent,
  personalities,
  runtimes,
  onRefreshRuntimes,
  onSave,
  onDelete,
  onClose,
}: Props) {
  const t = useT();
  const [confirming, setConfirming] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function confirmDelete() {
    setError(null);
    onDelete().catch((e: unknown) => {
      setError(errorMessage(t, e));
      setConfirming(false);
    });
  }

  return (
    <section
      className={styles.panel}
      aria-label={t('workspace.editPanel.title', { name: agent.name })}
    >
      <h2 className={styles.panelTitle}>{t('workspace.editPanel.title', { name: agent.name })}</h2>
      <AgentForm
        key={agent.id}
        initial={agent}
        personalities={personalities}
        runtimes={runtimes}
        onRefreshRuntimes={onRefreshRuntimes}
        onSubmit={onSave}
        onSaved={onClose}
        onCancel={onClose}
      />
      {error && (
        <p role="alert" className={styles.notice}>
          {error}
        </p>
      )}
      <div className={styles.panelActions}>
        {confirming ? (
          <>
            <span>{t('workspace.editPanel.confirm', { name: agent.name })}</span>
            <button type="button" className={styles.danger} onClick={confirmDelete}>
              {t('common.confirmDelete')}
            </button>
            <button
              type="button"
              className={styles.linkButton}
              onClick={() => {
                setConfirming(false);
              }}
            >
              {t('common.cancel')}
            </button>
          </>
        ) : (
          <button
            type="button"
            className={styles.danger}
            onClick={() => {
              setConfirming(true);
            }}
          >
            {t('workspace.editPanel.delete')}
          </button>
        )}
      </div>
    </section>
  );
}
