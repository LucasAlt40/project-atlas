import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
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
  const [query, setQuery] = useState('');

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
  const needle = query.trim().toLowerCase();
  const visible = needle
    ? agents.filter((a) => `${a.name} ${a.modelId} ${a.runtimeId}`.toLowerCase().includes(needle))
    : agents;
  const runtimeName = (id: string) =>
    (runtimes.status === 'ready'
      ? runtimes.runtimes.find((r) => r.runtime.id === id)?.runtime.name
      : undefined) ?? id;

  return (
    <section className={styles.page}>
      {!creating && !editing && (
        <header className={styles.pageHeader}>
          <div className={styles.headerText}>
            <div className={styles.titleRow}>
              <h1 className={styles.title}>{t('agents.title')}</h1>
              <span className={styles.countChip}>{t('agents.count', { n: agents.length })}</span>
            </div>
            <p className={styles.subtitle}>{t('agents.subtitle')}</p>
          </div>
          <div className={styles.headerActions}>
            <label className={styles.search}>
              <Icon name="search" size={16} />
              <input
                type="search"
                value={query}
                placeholder={t('agents.search')}
                aria-label={t('agents.search')}
                onChange={(e) => {
                  setQuery(e.target.value);
                }}
              />
            </label>
            <Button
              onClick={() => {
                setEditingId(null);
                setCreating(true);
              }}
            >
              <Icon name="plus" size={16} />
              {t('agents.create')}
            </Button>
          </div>
        </header>
      )}

      {notice && (
        <p role="status" className={styles.muted}>
          {notice}
        </p>
      )}

      {(creating || editing) && (
        <header className={styles.headerText}>
          <span className={styles.crumb}>{t('agents.title')}</span>
          <h1 className={styles.title}>
            {editing ? t('agents.form.edit', { name: editing.name }) : t('agents.create')}
          </h1>
        </header>
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

      {!creating && !editing && (
        <AgentList
          agents={visible}
          filtered={needle !== ''}
          personalities={personalities}
          runtimeName={runtimeName}
          onEdit={(id) => {
            setCreating(false);
            setEditingId(id);
          }}
          onDelete={removeAgent}
        />
      )}
    </section>
  );
}
