import { useState } from 'react';
import { Badge, type BadgeTone } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import type { Agent } from '@/features/agents/types';
import type { TranslationKey } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import type { McpConnectionDto, McpGrantDto } from '@/lib/tauri/commands';
import { commandLine, statusOf, type HubStatus } from '../../model/hub';
import styles from './McpHub.module.css';

/** The one map from what the hub knows of a server to the badge that says so. */
const STATUS_TONE: Record<HubStatus, BadgeTone> = {
  off: 'neutral',
  unexamined: 'neutral',
  connected: 'success',
  failed: 'danger',
  needs_auth: 'warning',
  pending: 'warning',
  unknown: 'neutral',
};

export function StatusPill({ status }: { status: HubStatus }) {
  const { t } = useI18n();
  return (
    <Badge dot tone={STATUS_TONE[status]}>
      {t(`hub.status.${status}` as TranslationKey)}
    </Badge>
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
  const problem = status === 'failed' || status === 'needs_auth';
  const warningCount = missingSecrets.length + deliveryNotes.length + (problem ? 1 : 0);
  // The warning that matters most colours the indicator, even while the list is folded.
  const warningTone: BadgeTone =
    status === 'failed' || deliveryNotes.some((n) => n.kind === 'none') ? 'danger' : 'warning';
  const [warningsOpen, setWarningsOpen] = useState(false);
  const examined = connection.discovery
    ? t('hub.card.examined', {
        when: new Date(connection.discovery.discoveredAt).toLocaleString(language),
        runtime: connection.discovery.runtimeId,
      })
    : null;
  const summary = [
    connection.discovery ? t('hub.card.tools', { n: tools.length }) : t('hub.card.notExamined'),
    names.length === 0 ? t('hub.card.nobody') : t('hub.card.agents', { names: names.join(', ') }),
    examined,
  ].filter((part): part is string => part !== null);
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
          <strong className={styles.cardNameText}>{connection.name}</strong>
          <StatusPill status={status} />
          {connection.required && <Badge tone="warning">{t('integrations.required')}</Badge>}
        </button>
        <Button
          variant="ghost"
          size="sm"
          disabled={busy || !connection.enabled || missingSecrets.length > 0}
          onClick={onTest}
        >
          {t('hub.card.test')}
        </Button>
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

      <code className={styles.command} title={commandLine(connection)}>
        {commandLine(connection)}
      </code>
      <p className={styles.summary}>
        {summary.map((part) => (
          <span key={part}>{part}</span>
        ))}
      </p>

      {warningCount > 0 && (
        <div className={styles.warnings} data-tone={warningTone}>
          <button
            type="button"
            className={styles.warningsToggle}
            aria-expanded={warningsOpen}
            onClick={() => {
              setWarningsOpen((open) => !open);
            }}
          >
            <Icon name="warning" size={14} />
            {t(warningCount === 1 ? 'hub.card.warningsOne' : 'hub.card.warnings', {
              n: warningCount,
            })}
            <Icon name={warningsOpen ? 'chevronUp' : 'chevronDown'} size={14} />
          </button>
          {warningsOpen && (
            <ul className={styles.warningList}>
              {missingSecrets.length > 0 && (
                <li role="note">
                  {t('hub.card.secretMissing', { names: missingSecrets.join(', ') })}
                </li>
              )}
              {deliveryNotes.map((note) => (
                <li
                  key={`${note.agent}-${note.kind}`}
                  role={note.kind === 'none' ? 'alert' : 'note'}
                >
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
                </li>
              ))}
              {problem && <li role="note">{t(`hub.card.problem.${status}` as TranslationKey)}</li>}
            </ul>
          )}
        </div>
      )}
    </li>
  );
}
