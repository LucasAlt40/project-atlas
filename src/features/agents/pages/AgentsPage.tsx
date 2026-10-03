import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import { AgentForm } from '../components/AgentForm';
import { AgentList } from '../components/AgentList';
import { useCatalog } from '../hooks/useCatalog';
import type { Agent } from '../types';
import styles from './Pages.module.css';

interface Props {
  /** Open the "create agent" form first, optionally with a personality preselected. */
  startCreating?: { personalityId?: string } | undefined;
  /** Called after an agent is created, so the caller can place it in the workspace. */
  onAgentCreated?: (agent: Agent) => void;
  /** A message from whoever handled `onAgentCreated`. */
  notice?: string | null;
}

/** Agents are global configuration: personality + runtime + model + instructions. */
export function AgentsPage({ startCreating, onAgentCreated, notice }: Props) {
  const t = useT();
  const { catalog, runtimes, refreshRuntimes, addAgent, editAgent, removeAgent } = useCatalog();
  const [creating, setCreating] = useState(startCreating !== undefined);
  const [editingId, setEditingId] = useState<string | null>(null);

  if (catalog.status === 'loading') {
    return <p className={styles.muted}>{t('common.loadingCore')}</p>;
  }
  if (catalog.status === 'error') {
    return (
      <p role="alert" className={styles.error}>
        {t('common.coreUnavailable', { message: errorMessage(t, catalog.error) })}
      </p>
    );
  }

  const { personalities, agents } = catalog;
  const editing = agents.find((a) => a.id === editingId);
  const runtimeName = (id: string) =>
    (runtimes.status === 'ready'
      ? runtimes.runtimes.find((r) => r.runtime.id === id)?.runtime.name
      : undefined) ?? id;

  return (
    <section className={styles.page}>
      <header className={styles.header}>
        <h1 className={styles.title}>{t('agents.title')}</h1>
        {!creating && (
          <Button
            onClick={() => {
              setEditingId(null);
              setCreating(true);
            }}
          >
            {t('agents.create')}
          </Button>
        )}
      </header>

      {notice && (
        <p role="status" className={styles.muted}>
          {notice}
        </p>
      )}

      {creating && (
        <AgentForm
          personalities={personalities}
          runtimes={runtimes}
          initialPersonalityId={startCreating?.personalityId ?? ''}
          onRefreshRuntimes={refreshRuntimes}
          onSubmit={addAgent}
          onSaved={(agent) => {
            setCreating(false);
            onAgentCreated?.(agent);
          }}
          onCancel={() => {
            setCreating(false);
          }}
        />
      )}

      {editing && (
        <AgentForm
          key={editing.id}
          initial={editing}
          personalities={personalities}
          runtimes={runtimes}
          onRefreshRuntimes={refreshRuntimes}
          onSubmit={(input) => editAgent(editing.id, input)}
          onSaved={() => {
            setEditingId(null);
          }}
          onCancel={() => {
            setEditingId(null);
          }}
        />
      )}

      <AgentList
        agents={agents}
        personalities={personalities}
        runtimeName={runtimeName}
        onEdit={(id) => {
          setCreating(false);
          setEditingId(id);
        }}
        onDelete={removeAgent}
      />
    </section>
  );
}
