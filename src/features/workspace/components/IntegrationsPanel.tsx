import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import type { Agent, RuntimeStatus } from '@/features/agents/types';
import type { TranslationKey } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import type { McpPresetDto, McpTransportDto } from '@/lib/tauri/commands';
import { useMcpConnections } from '../hooks/useMcpConnections';
import { addMcpConnection, addMcpPreset } from '../services/mcpService';
import { ConnectionCard } from './ConnectionCard';
import styles from './Integrations.module.css';

interface Props {
  workspaceId: string;
  agents: Agent[];
  runtimes: RuntimeStatus[];
  onClose: () => void;
}

/**
 * The workspace's integrations (MCP servers): what Atlas knows by name, the connections the user
 * made and who may use them. Adding anything creates a connection that is off, granted to nobody
 * and not started; each step after that is the user's.
 */
export function IntegrationsPanel({ workspaceId, agents, runtimes, onClose }: Props) {
  const { t } = useI18n();
  const { state, reload } = useMcpConnections(workspaceId);
  const [error, setError] = useState<string | null>(null);

  function run(action: () => Promise<unknown>) {
    setError(null);
    action()
      .then(reload)
      .catch((e: unknown) => {
        setError(errorMessage(t, e));
      });
  }

  return (
    <section className={styles.panel} aria-label={t('integrations.title')}>
      <div className={styles.cardHead}>
        <h2 className={styles.title}>{t('integrations.title')}</h2>
        <button type="button" className={styles.linkButton} onClick={onClose}>
          {t('common.close')}
        </button>
      </div>
      <p className={styles.note}>{t('integrations.note')}</p>
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
      {state.status === 'ready' && (
        <>
          <h3 className={styles.heading}>{t('integrations.catalog')}</h3>
          <ul className={styles.list}>
            {state.value.catalog.map((preset) => (
              <PresetRow
                key={preset.id}
                preset={preset}
                added={state.value.overview.connections.some(
                  (c) => c.name === preset.connectionName,
                )}
                onAdd={() => {
                  run(() => addMcpPreset(workspaceId, preset.id));
                }}
              />
            ))}
          </ul>

          <h3 className={styles.heading}>{t('integrations.connections')}</h3>
          {state.value.overview.connections.length === 0 && (
            <p className={styles.muted}>{t('integrations.none')}</p>
          )}
          <ul className={styles.list}>
            {state.value.overview.connections.map((connection) => {
              const preset = state.value.catalog.find(
                (p) => p.connectionName === connection.name && p.availability === 'ready',
              );
              return (
                <ConnectionCard
                  key={connection.id}
                  connection={connection}
                  grants={state.value.overview.grants}
                  agents={agents}
                  runtimes={runtimes}
                  riskNoteKey={
                    preset ? (`integrations.preset.${preset.id}.risk` as TranslationKey) : undefined
                  }
                  onChanged={reload}
                />
              );
            })}
          </ul>

          <CustomForm
            onAdd={(name, transport) => {
              run(() => addMcpConnection(workspaceId, name, transport, false));
            }}
          />
        </>
      )}
    </section>
  );
}

function PresetRow({
  preset,
  added,
  onAdd,
}: {
  preset: McpPresetDto;
  added: boolean;
  onAdd: () => void;
}) {
  const { t } = useI18n();
  const planned = preset.availability !== 'ready';
  return (
    <li className={styles.card} aria-label={preset.connectionName}>
      <div className={styles.cardHead}>
        <strong>{t(`integrations.preset.${preset.id}.name` as TranslationKey)}</strong>
        <span className={styles.muted}>{t('integrations.riskHigh')}</span>
      </div>
      <p className={styles.muted}>
        {t(`integrations.preset.${preset.id}.about` as TranslationKey)}
      </p>
      {planned && <p className={styles.warning}>{t('integrations.needsHttp')}</p>}
      {preset.command && (
        <p className={styles.muted}>
          {t('integrations.wouldRun')} <code className={styles.mono}>{preset.command}</code>
        </p>
      )}
      {preset.pinnedVersion && (
        <p className={styles.muted}>
          {t('integrations.pinned', { version: preset.pinnedVersion })}
        </p>
      )}
      {preset.requirements.length > 0 && (
        <ul className={styles.list}>
          {preset.requirements.map((r) => (
            <li key={r.requirement} className={styles.muted}>
              {r.found ? '✓' : '·'} {t(`integrations.req.${r.requirement}` as TranslationKey)} —{' '}
              {r.found ? t('integrations.reqFound') : t('integrations.reqNotFound')}
            </li>
          ))}
        </ul>
      )}
      <div className={styles.row}>
        <Button variant="secondary" disabled={planned || added} onClick={onAdd}>
          {added ? t('integrations.added') : t('integrations.add')}
        </Button>
        <a href={preset.sourceUrl} target="_blank" rel="noreferrer" className={styles.linkButton}>
          {t('integrations.source')}
        </a>
      </div>
    </li>
  );
}

/** A server the user runs themselves: a program and its arguments, never a shell command line. */
function CustomForm({ onAdd }: { onAdd: (name: string, transport: McpTransportDto) => void }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [name, setName] = useState('');
  const [executable, setExecutable] = useState('');
  const [args, setArgs] = useState('');
  const [plain, setPlain] = useState('');
  const [secrets, setSecrets] = useState('');

  if (!open) {
    return (
      <div className={styles.row}>
        <button
          type="button"
          className={styles.linkButton}
          onClick={() => {
            setOpen(true);
          }}
        >
          {t('integrations.custom')}
        </button>
      </div>
    );
  }

  const lines = (text: string) =>
    text
      .split('\n')
      .map((l) => l.trim())
      .filter(Boolean);
  const env: NonNullable<Extract<McpTransportDto, { kind: 'stdio' }>['env']> = [
    ...lines(plain).map((line) => {
      const at = line.indexOf('=');
      return {
        name: at < 0 ? line : line.slice(0, at),
        value: { kind: 'plain' as const, value: at < 0 ? '' : line.slice(at + 1) },
      };
    }),
    ...lines(secrets).map((secret) => ({ name: secret, value: { kind: 'secret' as const } })),
  ];

  return (
    <form
      className={styles.card}
      aria-label={t('integrations.custom')}
      onSubmit={(e) => {
        e.preventDefault();
        onAdd(name.trim(), {
          kind: 'stdio',
          executable: executable.trim(),
          args: lines(args),
          env,
        });
        setOpen(false);
        setName('');
        setExecutable('');
        setArgs('');
        setPlain('');
        setSecrets('');
      }}
    >
      <p className={styles.muted}>{t('integrations.customNote')}</p>
      <label className={styles.field}>
        {t('integrations.customName')}
        <input
          value={name}
          onChange={(e) => {
            setName(e.target.value);
          }}
        />
      </label>
      <label className={styles.field}>
        {t('integrations.customExecutable')}
        <input
          value={executable}
          onChange={(e) => {
            setExecutable(e.target.value);
          }}
        />
      </label>
      <label className={styles.field}>
        {t('integrations.customArgs')}
        <textarea
          rows={3}
          value={args}
          onChange={(e) => {
            setArgs(e.target.value);
          }}
        />
      </label>
      <label className={styles.field}>
        {t('integrations.customEnv')}
        <textarea
          rows={2}
          value={plain}
          onChange={(e) => {
            setPlain(e.target.value);
          }}
        />
      </label>
      <label className={styles.field}>
        {t('integrations.customSecrets')}
        <textarea
          rows={2}
          value={secrets}
          onChange={(e) => {
            setSecrets(e.target.value);
          }}
        />
      </label>
      <div className={styles.row}>
        <Button type="submit" disabled={name.trim() === '' || executable.trim() === ''}>
          {t('integrations.customAdd')}
        </Button>
        <button
          type="button"
          className={styles.linkButton}
          onClick={() => {
            setOpen(false);
          }}
        >
          {t('common.cancel')}
        </button>
      </div>
    </form>
  );
}
