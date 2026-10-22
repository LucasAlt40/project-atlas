import { useEffect, useMemo, useRef, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { Modal } from '@/components/ui/Modal';
import { OverflowMenu } from '@/components/ui/OverflowMenu';
import { PageHeader } from '@/components/ui/PageHeader';
import { SegmentedControl } from '@/components/ui/SegmentedControl';
import { useCatalog } from '@/features/agents/hooks/useCatalog';
import type { Agent, RuntimeStatus } from '@/features/agents/types';
import type { TranslationKey } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import type { McpGrantDto } from '@/lib/tauri/commands';
import { AddServer } from '../components/hub/AddServer';
import { ConfigIo } from '../components/hub/ConfigIo';
import { Inspector } from '../components/hub/Inspector';
import { ServerCard } from '../components/hub/ServerCard';
import { SyncRuntimes } from '../components/hub/SyncRuntimes';
import { useMcpConnections } from '../hooks/useMcpConnections';
import { useWorkspace } from '../hooks/WorkspaceProvider';
import { envOf, matches, metricsOf, type HubTab } from '../model/hub';
import { deliveryTo } from '../components/connectionParts';
import {
  addMcpPreset,
  exportMcpConfig,
  probeMcpConnection,
  setMcpConnectionEnabled,
} from '../services/mcpService';
import styles from '../components/hub/McpHub.module.css';

type Dialog = 'add' | 'config' | 'sync' | null;

/** The agents a server is granted to that will not simply be handed it, and what happens instead. */
function deliveryNotesFor(
  connectionId: string,
  grants: McpGrantDto[],
  agents: Agent[],
  runtimes: RuntimeStatus[],
): { agent: string; runtime: string; kind: 'cli' | 'none' }[] {
  const notes: { agent: string; runtime: string; kind: 'cli' | 'none' }[] = [];
  for (const grant of grants.filter((g) => g.connectionId === connectionId)) {
    const agent = agents.find((a) => a.id === grant.agentId);
    if (!agent) continue;
    const kind = deliveryTo(agent, runtimes);
    if (kind === 'handed' || notes.some((n) => n.agent === agent.name)) continue;
    notes.push({
      agent: agent.name,
      runtime:
        runtimes.find((r) => r.runtime.id === agent.runtimeId)?.runtime.name ?? agent.runtimeId,
      kind,
    });
  }
  return notes;
}

/**
 * MCP Hub: the servers of the active workspace in one place. What each is, how it was last seen,
 * who may use it, how to bring more in (catalogue, the MCP Registry, JSON in any CLI's format) and
 * how to keep every CLI's own configuration in step. Nothing here starts, switches on or grants
 * anything by itself.
 */
export function McpHubPage() {
  const { t } = useI18n();
  const workspace = useWorkspace();
  const active = workspace.active;
  if (!active) {
    return <p className={styles.muted}>{t('hub.noWorkspace')}</p>;
  }
  return <Hub key={active.id} workspaceId={active.id} />;
}

function Hub({ workspaceId }: { workspaceId: string }) {
  const { t } = useI18n();
  const { catalog: agentCatalog, runtimes: runtimeState } = useCatalog();
  const { state, reload } = useMcpConnections(workspaceId);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [confirmingOn, setConfirmingOn] = useState<string | null>(null);
  const [dialog, setDialog] = useState<Dialog>(null);
  const [tab, setTab] = useState<HubTab>('all');
  const [query, setQuery] = useState('');
  const [onlyConnected, setOnlyConnected] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pinging, setPinging] = useState<{ done: number; total: number } | null>(null);
  const search = useRef<HTMLInputElement>(null);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        search.current?.focus();
      }
    }
    document.addEventListener('keydown', onKeyDown);
    return () => {
      document.removeEventListener('keydown', onKeyDown);
    };
  }, []);

  const agents = agentCatalog.status === 'ready' ? agentCatalog.agents : [];
  const runtimes = runtimeState.status === 'ready' ? runtimeState.runtimes : [];
  const value = state.status === 'ready' ? state.value : undefined;
  const connections = useMemo(() => value?.overview.connections ?? [], [value]);
  const grants = useMemo(() => value?.overview.grants ?? [], [value]);

  const metrics = metricsOf(connections, grants);
  const visible = connections.filter((c) => matches(c, { tab, query, onlyConnected }));
  const selected = connections.find((c) => c.id === selectedId) ?? visible[0];
  const counts: Record<HubTab, number> = {
    all: connections.length,
    local: connections.filter((c) => c.transport.kind === 'stdio').length,
    remote: connections.filter((c) => c.transport.kind !== 'stdio').length,
  };

  function run(action: () => Promise<unknown>) {
    setBusy(true);
    setError(null);
    action()
      .then(reload)
      .catch((cause: unknown) => {
        setError(errorMessage(t, cause));
      })
      .finally(() => {
        setBusy(false);
      });
  }

  /** Looks at every switched-on server through the first runtime that can, one after the other. */
  function pingAll() {
    const runtime = runtimes.find(
      (r) =>
        r.runtime.capabilities.mcp === 'supported' &&
        r.runtime.capabilities.mcpFeatures.probe !== 'none',
    );
    const ready = connections.filter(
      (c) => c.enabled && envOf(c).every((e) => e.value.kind !== 'secret' || e.name in c.secrets),
    );
    if (!runtime || ready.length === 0) {
      setError(t(runtime ? 'hub.ping.nothing' : 'integrations.noRuntime'));
      return;
    }
    setError(null);
    setPinging({ done: 0, total: ready.length });
    void (async () => {
      for (const [index, connection] of ready.entries()) {
        try {
          await probeMcpConnection(connection.id, runtime.runtime.id);
        } catch {
          // A server that does not answer is a result, not a reason to stop: the card says so.
        }
        setPinging({ done: index + 1, total: ready.length });
      }
      setPinging(null);
      reload();
    })();
  }

  function toggle(id: string, enabled: boolean) {
    setSelectedId(id);
    if (enabled) {
      setConfirmingOn(null);
      run(() => setMcpConnectionEnabled(id, false));
    } else {
      setConfirmingOn(id);
    }
  }

  const tabs: { id: HubTab; label: string; disabled?: boolean }[] = [
    { id: 'all', label: t('hub.filter.all') },
    { id: 'local', label: t('hub.filter.local') },
    { id: 'remote', label: t('hub.filter.remote'), disabled: counts.remote === 0 },
  ];
  const metricCards: { label: string; value: number; hint: string; tone?: string }[] = [
    { label: t('hub.metric.total'), value: metrics.total, hint: t('hub.metric.totalHint') },
    {
      label: t('hub.metric.connected'),
      value: metrics.connected,
      hint:
        metrics.total - metrics.off > 0
          ? t('hub.metric.connectedHint', {
              pct: Math.round((metrics.connected / (metrics.total - metrics.off)) * 100),
            })
          : '—',
      tone: 'ok',
    },
    {
      label: t('hub.metric.degraded'),
      value: metrics.degraded,
      hint: t('hub.metric.degradedHint'),
      tone: metrics.degraded > 0 ? 'warn' : '',
    },
    { label: t('hub.metric.off'), value: metrics.off, hint: t('hub.metric.offHint') },
    { label: t('hub.metric.tools'), value: metrics.tools, hint: t('hub.metric.toolsHint') },
    { label: t('hub.metric.grants'), value: metrics.grants, hint: t('hub.metric.grantsHint') },
  ];

  const presetFor = (name: string) =>
    value?.catalog.find((p) => p.connectionName === name && p.availability === 'ready');

  return (
    <section className={styles.page} aria-label={t('hub.title')}>
      <PageHeader
        title={t('hub.title')}
        description={t('hub.subtitle')}
        actions={
          <>
            <Button
              variant="secondary"
              disabled={pinging !== null || connections.length === 0}
              onClick={pingAll}
            >
              <Icon name="refresh" size={14} />
              {pinging
                ? t('hub.ping.running', { done: pinging.done, total: pinging.total })
                : t('hub.ping')}
            </Button>
            <Button
              onClick={() => {
                setDialog('add');
              }}
            >
              <Icon name="plus" size={14} />
              {t('hub.add.open')}
            </Button>
            <OverflowMenu
              label={t('hub.config.menu')}
              items={[
                {
                  label: t('hub.sync.open'),
                  icon: 'refresh',
                  onSelect: () => {
                    setDialog('sync');
                  },
                },
                {
                  label: t('hub.config.importExport'),
                  icon: 'braces',
                  onSelect: () => {
                    setDialog('config');
                  },
                },
              ]}
            />
          </>
        }
      />

      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
      {state.status === 'loading' && <p className={styles.muted}>…</p>}
      {state.status === 'error' && (
        <p role="alert" className={styles.error}>
          {errorMessage(t, state.error)}
        </p>
      )}

      {value && (
        <>
          {connections.length > 0 && (
            <>
              <ul className={styles.metrics} aria-label={t('hub.metric.label')}>
                {metricCards.map((card) => (
                  <li
                    key={card.label}
                    className={styles.metric}
                    data-tone={card.tone}
                    title={card.hint}
                  >
                    <span className={styles.metricValue}>{card.value}</span>{' '}
                    <span className={styles.metricLabel}>{card.label}</span>
                    {card.tone === 'ok' && card.hint !== '—' && (
                      <span className={styles.metricHint}> ({card.hint})</span>
                    )}
                  </li>
                ))}
              </ul>

              <div className={styles.controls}>
                <SegmentedControl
                  label={t('hub.filter.label')}
                  value={tab}
                  onChange={setTab}
                  segments={tabs.map((item) => ({
                    value: item.id,
                    label: `${item.label} (${String(counts[item.id])})`,
                    ...(item.disabled ? { disabled: true, title: t('hub.filter.remoteSoon') } : {}),
                  }))}
                />
                <div className={styles.row}>
                  <label className={styles.search}>
                    <Icon name="search" size={14} />
                    <input
                      ref={search}
                      value={query}
                      aria-label={t('hub.search')}
                      placeholder={t('hub.search')}
                      onChange={(e) => {
                        setQuery(e.target.value);
                      }}
                    />
                    <kbd>⌘K</kbd>
                  </label>
                  <label className={styles.check}>
                    <span className={styles.switch}>
                      <input
                        type="checkbox"
                        checked={onlyConnected}
                        onChange={(e) => {
                          setOnlyConnected(e.target.checked);
                        }}
                      />
                      <span aria-hidden="true" />
                    </span>
                    {t('hub.onlyConnected')}
                  </label>
                </div>
              </div>
            </>
          )}

          {connections.length === 0 ? (
            <div className={styles.empty}>
              <span className={styles.emptyIcon} aria-hidden="true">
                <Icon name="hub" size={22} />
              </span>
              <p>{t('hub.empty')}</p>
              <div className={styles.row}>
                <Button
                  onClick={() => {
                    setDialog('add');
                  }}
                >
                  {t('hub.add.open')}
                </Button>
                <Button
                  variant="secondary"
                  onClick={() => {
                    setDialog('sync');
                  }}
                >
                  {t('hub.sync.open')}
                </Button>
              </div>
            </div>
          ) : (
            <div className={styles.split}>
              <ul className={styles.list} aria-label={t('hub.list')}>
                {visible.length === 0 && <li className={styles.muted}>{t('hub.noMatch')}</li>}
                {visible.map((connection) => (
                  <ServerCard
                    key={connection.id}
                    connection={connection}
                    grants={grants}
                    agents={agents}
                    selected={selected?.id === connection.id}
                    busy={busy}
                    deliveryNotes={deliveryNotesFor(connection.id, grants, agents, runtimes)}
                    onCopyCliCommand={() => {
                      void exportMcpConfig(workspaceId, 'antigravity', [connection.id])
                        .then((text) => navigator.clipboard.writeText(text))
                        .catch(() => {
                          setError(t('hub.config.copyFailed'));
                        });
                    }}
                    missingSecrets={envOf(connection)
                      .filter((e) => e.value.kind === 'secret' && !(e.name in connection.secrets))
                      .map((e) => e.name)}
                    onSelect={() => {
                      setSelectedId(connection.id);
                    }}
                    onToggle={() => {
                      toggle(connection.id, connection.enabled);
                    }}
                    onTest={() => {
                      const runtime = runtimes.find(
                        (r) =>
                          r.runtime.capabilities.mcp === 'supported' &&
                          r.runtime.capabilities.mcpFeatures.probe !== 'none',
                      );
                      if (!runtime) {
                        setError(t('integrations.noRuntime'));
                        return;
                      }
                      run(() => probeMcpConnection(connection.id, runtime.runtime.id));
                    }}
                  />
                ))}
              </ul>
              {selected ? (
                <Inspector
                  key={selected.id}
                  connection={selected}
                  grants={grants}
                  agents={agents}
                  runtimes={runtimes}
                  workflows={value.workflows}
                  riskNoteKey={
                    presetFor(selected.name)
                      ? (`integrations.preset.${presetFor(selected.name)?.id ?? ''}.risk` as TranslationKey)
                      : undefined
                  }
                  confirmingOn={confirmingOn === selected.id && !selected.enabled}
                  busy={busy}
                  run={run}
                  onConfirmOn={(on) => {
                    setConfirmingOn(on ? selected.id : null);
                  }}
                  onExport={() => {
                    setDialog('config');
                  }}
                />
              ) : (
                <p className={styles.muted}>{t('hub.inspector.empty')}</p>
              )}
            </div>
          )}

          {dialog === 'add' && (
            <Modal
              label={t('hub.add.title')}
              title={t('hub.add.title')}
              size="lg"
              footer={
                <Button
                  variant="secondary"
                  onClick={() => {
                    setDialog(null);
                  }}
                >
                  {t('common.close')}
                </Button>
              }
              onClose={() => {
                setDialog(null);
              }}
            >
              <AddServer
                workspaceId={workspaceId}
                catalog={value.catalog}
                existing={connections.map((c) => c.name)}
                onAddPreset={(presetId) => {
                  run(() => addMcpPreset(workspaceId, presetId));
                }}
                onAdded={reload}
              />
            </Modal>
          )}
          {dialog === 'config' && (
            <Modal
              label={t('hub.config.title')}
              title={t('hub.config.title')}
              size="md"
              onClose={() => {
                setDialog(null);
              }}
            >
              <ConfigIo
                workspaceId={workspaceId}
                connections={connections}
                selectedId={selected?.id ?? null}
                onChanged={reload}
              />
            </Modal>
          )}
          {dialog === 'sync' && (
            <Modal
              label={t('hub.sync.title')}
              title={t('hub.sync.title')}
              size="lg"
              onClose={() => {
                setDialog(null);
              }}
            >
              <SyncRuntimes workspaceId={workspaceId} onChanged={reload} />
            </Modal>
          )}
        </>
      )}
    </section>
  );
}
