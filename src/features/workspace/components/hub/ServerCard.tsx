import { Icon } from '@/components/ui/Icon';
import type { Agent } from '@/features/agents/types';
import type { TranslationKey } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import type { McpConnectionDto, McpGrantDto } from '@/lib/tauri/commands';
import { commandLine, statusOf, type HubStatus } from '../../model/hub';
import styles from './McpHub.module.css';

export function StatusPill({ status }: { status: HubStatus }) {
  const { t } = useI18n();
  return (
    <span className={styles.pill} data-status={status}>
      <span className={styles.dot} aria-hidden="true" />
      {t(`hub.status.${status}` as TranslationKey)}
    </span>
  );
}

interface Props {
  connection: McpConnectionDto;
  grants: McpGrantDto[];
  agents: Agent[];
  selected: boolean;
  busy: boolean;
  /** Names of secrets that were declared and have no value. */
  missingSecrets: string[];
  /** Agents this server is granted to whose runtime does not simply receive it. */
  deliveryNotes: { agent: string; runtime: string; kind: 'cli' | 'none' }[];
  onCopyCliCommand: () => void;
  onSelect: () => void;
  onToggle: () => void;
  onTest: () => void;
}

/** One server of the registry: what it is, how it was last seen, who may use it. */
export function ServerCard({
  connection,
  grants,
  agents,
  selected,
  busy,
  missingSecrets,
  deliveryNotes,
  onCopyCliCommand,
  onSelect,
  onToggle,
  onTest,
}: Props) {
  const { t, language } = useI18n();
  const status = statusOf(connection);
  const mine = grants.filter((g) => g.connectionId === connection.id);
  const names = [
    ...new Set(
      mine.map((g) =>
        g.agentId === null
          ? t('integrations.anyAgent')
          : (agents.find((a) => a.id === g.agentId)?.name ?? '?'),
      ),
    ),
  ];
  const tools = connection.discovery?.tools ?? [];
  const kind = connection.transport.kind === 'stdio' ? 'stdio' : 'http';
  return (
    <li
      className={styles.card}
      data-selected={selected}
      data-status={status}
      aria-label={connection.name}
    >
      <div className={styles.cardHead}>
        <button
          type="button"
          className={styles.cardTitle}
          aria-pressed={selected}
          aria-label={t('hub.card.inspectNamed', { name: connection.name })}
          onClick={onSelect}
        >
          <span className={styles.cardIcon} aria-hidden="true">
            <Icon name="plug" size={18} />
          </span>
          <span className={styles.cardName}>
            <span className={styles.nameLine}>
              <strong>{connection.name}</strong>
              <span className={styles.chip}>{kind}</span>
              {connection.required && (
                <span className={styles.chip}>{t('integrations.required')}</span>
              )}
            </span>
            <code className={styles.mono}>{commandLine(connection)}</code>
          </span>
        </button>
        <div className={styles.cardState}>
          <StatusPill status={status} />
          <label className={styles.switch}>
            <input
              type="checkbox"
              role="switch"
              checked={connection.enabled}
              disabled={busy}
              aria-label={t('hub.card.switch', { name: connection.name })}
              onChange={onToggle}
            />
            <span aria-hidden="true" />
          </label>
        </div>
      </div>

      <div className={styles.tags}>
        <span className={styles.tag}>
          <Icon name="wrench" size={12} />{' '}
          {connection.discovery
            ? t('hub.card.tools', { n: tools.length })
            : t('hub.card.notExamined')}
        </span>
        <span className={styles.tag}>
          <Icon name="agents" size={12} />{' '}
          {names.length === 0
            ? t('hub.card.nobody')
            : t('hub.card.agents', { names: names.join(', ') })}
        </span>
        {connection.discovery && (
          <span className={styles.tag}>
            {t('hub.card.examined', {
              when: new Date(connection.discovery.discoveredAt).toLocaleString(language),
              runtime: connection.discovery.runtimeId,
            })}
          </span>
        )}
      </div>

      {missingSecrets.length > 0 && (
        <p className={styles.callout} role="note">
          <Icon name="warning" size={14} />{' '}
          {t('hub.card.secretMissing', { names: missingSecrets.join(', ') })}
        </p>
      )}
      {deliveryNotes.map((note) => (
        <p
          key={`${note.agent}-${note.kind}`}
          className={styles.callout}
          role={note.kind === 'none' ? 'alert' : 'note'}
        >
          <Icon name="warning" size={14} />{' '}
          {t(`hub.card.delivery.${note.kind}` as TranslationKey, {
            agent: note.agent,
            runtime: note.runtime,
          })}
          {note.kind === 'cli' && (
            <>
              {' '}
              <button type="button" className={styles.link} onClick={onCopyCliCommand}>
                {t('hub.card.copyCliCommand')}
              </button>
            </>
          )}
        </p>
      ))}
      {(status === 'failed' || status === 'needs_auth') && (
        <p className={styles.callout} role="note">
          <Icon name="warning" size={14} /> {t(`hub.card.problem.${status}` as TranslationKey)}
        </p>
      )}

      <div className={styles.cardFoot}>
        <span className={styles.muted}>
          {connection.enabled ? t('integrations.on') : t('integrations.off')}
        </span>
        <div className={styles.row}>
          <button type="button" className={styles.ghost} onClick={onSelect}>
            {t('hub.card.inspect')}
          </button>
          <button
            type="button"
            className={styles.ghost}
            disabled={busy || !connection.enabled || missingSecrets.length > 0}
            onClick={onTest}
          >
            {t('hub.card.test')}
          </button>
        </div>
      </div>
    </li>
  );
}
