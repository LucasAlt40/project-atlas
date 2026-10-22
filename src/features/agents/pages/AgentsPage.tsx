import { useState } from 'react';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import { ImportDialog } from '@/features/portability';
import { OverflowMenu } from '@/components/ui/OverflowMenu';
import { useBreadcrumb } from '@/app/BreadcrumbContext';
import { PageHeader } from '@/components/ui/PageHeader';
import { SegmentedControl } from '@/components/ui/SegmentedControl';
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
  const { catalog, runtimes, refreshRuntimes, addAgent, editAgent, removeAgent, reloadCatalog } =
    useCatalog();
  const [creating, setCreating] = useState(startCreating !== undefined);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState<'all' | 'ready' | 'unavailable'>('all');
  const [importing, setImporting] = useState(false);
  const editingName =
    catalog.status === 'ready' ? catalog.agents.find((a) => a.id === editingId)?.name : undefined;
  useBreadcrumb([
    creating && t('agents.create'),
    editingName !== undefined && t('agents.form.edit', { name: editingName }),
  ]);

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
  const runtimeName = (id: string) =>
    (runtimes.status === 'ready'
      ? runtimes.runtimes.find((r) => r.runtime.id === id)?.runtime.name
      : undefined) ?? id;

  const runtimeReady = (id: string) =>
    runtimes.status === 'ready'
      ? runtimes.runtimes.find((r) => r.runtime.id === id)?.availability === 'ready'
      : undefined;

  const availability = (a: Agent) => runtimeReady(a.runtimeId);
  const inFilter = (a: Agent, f: typeof filter) =>
    f === 'all' || availability(a) === (f === 'ready');
  const visible = agents.filter(
    (a) =>
      inFilter(a, filter) &&
      (!needle || `${a.name} ${a.modelId} ${a.runtimeId}`.toLowerCase().includes(needle)),
  );
  const availabilityOf = (id: string) =>
    runtimes.status === 'ready'
      ? runtimes.runtimes.find((r) => r.runtime.id === id)?.availability
      : undefined;

  return (
    <section className={styles.page}>
      {!creating && !editing && (
        <>
          <PageHeader
            title={t('agents.title')}
            titleAside={<Badge mono>{String(agents.length)}</Badge>}
            actions={
              <>
                <Button
                  onClick={() => {
                    setEditingId(null);
                    setCreating(true);
                  }}
                >
                  <Icon name="plus" size={16} />
                  {t('agents.create')}
                </Button>
                <OverflowMenu
                  label={t('agents.more')}
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
              </>
            }
          />
          {agents.length > 0 && (
            <div className={styles.controls}>
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
              <SegmentedControl
                label={t('agents.filter.label')}
                value={filter}
                onChange={setFilter}
                segments={(['all', 'ready', 'unavailable'] as const).map((name) => ({
                  value: name,
                  label: `${t(`agents.filter.${name}`)} (${String(agents.filter((a) => inFilter(a, name)).length)})`,
                }))}
              />
            </div>
          )}
        </>
      )}

      {notice && (
        <p role="status" className={styles.muted}>
          {notice}
        </p>
      )}

      {(creating || editing) && (
        <div className={styles.formHead}>
          <PageHeader
            title={editing ? t('agents.form.edit', { name: editing.name }) : t('agents.create')}
            back={{
              label: t('agents.title'),
              onClick: () => {
                setCreating(false);
                setEditingId(null);
              },
            }}
          />
        </div>
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
          filtered={needle !== '' || filter !== 'all'}
          personalities={personalities}
          runtimeName={runtimeName}
          runtimeReady={runtimeReady}
          runtimeAvailability={availabilityOf}
          onEdit={(id) => {
            setCreating(false);
            setEditingId(id);
          }}
          onDelete={removeAgent}
          onCreate={() => {
            setEditingId(null);
            setCreating(true);
          }}
        />
      )}
      {importing && (
        <ImportDialog
          onImported={() => {
            void reloadCatalog();
          }}
          onClose={() => {
            setImporting(false);
          }}
        />
      )}
    </section>
  );
}
