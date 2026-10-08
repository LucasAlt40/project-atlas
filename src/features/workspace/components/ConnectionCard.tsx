import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import type { Agent, RuntimeStatus } from '@/features/agents/types';
import type { TranslationKey } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import type { McpConnectionDto, McpGrantDto } from '@/lib/tauri/commands';
import {
  clearMcpSecret,
  grantMcpConnection,
  probeMcpConnection,
  removeMcpConnection,
  revokeMcpGrant,
  setMcpConnectionEnabled,
  setMcpSecret,
} from '../services/mcpService';
import styles from './Integrations.module.css';

/** Programs that fetch and run code from the network the first time they start. */
const FETCHES_CODE = new Set(['npx', 'uvx', 'bunx', 'pnpx', 'dlx']);

interface Props {
  connection: McpConnectionDto;
  grants: McpGrantDto[];
  agents: Agent[];
  runtimes: RuntimeStatus[];
  /** What the catalogue says about this connection's risk, when it came from there. */
  riskNoteKey?: TranslationKey | undefined;
  onChanged: () => void;
}

/**
 * One connection: switch it on (a decision, with its consequence said first), look at what it
 * offers, store its secrets, and say which agents may use which of its tools. Nothing here starts
 * by itself, and nothing a person has not asked for is granted.
 */
export function ConnectionCard({
  connection,
  grants,
  agents,
  runtimes,
  riskNoteKey,
  onChanged,
}: Props) {
  const { t } = useI18n();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirming, setConfirming] = useState<'on' | 'remove' | null>(null);

  function run(action: () => Promise<unknown>) {
    setBusy(true);
    setError(null);
    action()
      .then(() => {
        setConfirming(null);
        onChanged();
      })
      .catch((e: unknown) => {
        setError(errorMessage(t, e));
      })
      .finally(() => {
        setBusy(false);
      });
  }

  const usable = runtimes.filter((r) => r.runtime.capabilities.mcp === 'supported');
  const stdio = connection.transport.kind === 'stdio' ? connection.transport : undefined;
  const program =
    stdio?.executable
      .split(/[\\/]/)
      .pop()
      ?.replace(/\.\w+$/, '') ?? '';
  const secretNames = (stdio?.env ?? [])
    .filter((e) => e.value.kind === 'secret')
    .map((e) => e.name);
  const missing = secretNames.filter((name) => !(name in connection.secrets));
  const mine = grants.filter((g) => g.connectionId === connection.id);

  return (
    <li className={styles.card} aria-label={connection.name}>
      <div className={styles.cardHead}>
        <strong>{connection.name}</strong>
        <span className={styles.muted}>
          {connection.enabled ? t('integrations.on') : t('integrations.off')} ·{' '}
          {connection.required ? t('integrations.required') : t('integrations.optional')}
        </span>
      </div>
      {stdio && (
        <code className={styles.mono}>{[stdio.executable, ...(stdio.args ?? [])].join(' ')}</code>
      )}
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}

      {/* Switching on: the consequence first. */}
      {confirming === 'on' ? (
        <div className={styles.warning} role="group" aria-label={t('integrations.confirmOn')}>
          <p>{t('integrations.confirmOnText')}</p>
          {riskNoteKey && <p>{t(riskNoteKey)}</p>}
          <div className={styles.row}>
            <Button
              disabled={busy}
              onClick={() => {
                run(() => setMcpConnectionEnabled(connection.id, true));
              }}
            >
              {t('integrations.confirmOn')}
            </Button>
            <button
              type="button"
              className={styles.linkButton}
              onClick={() => {
                setConfirming(null);
              }}
            >
              {t('common.cancel')}
            </button>
          </div>
        </div>
      ) : (
        <div className={styles.row}>
          {connection.enabled ? (
            <Button
              variant="secondary"
              disabled={busy}
              onClick={() => {
                run(() => setMcpConnectionEnabled(connection.id, false));
              }}
            >
              {t('integrations.switchOff')}
            </Button>
          ) : (
            <Button
              variant="secondary"
              disabled={busy}
              onClick={() => {
                setConfirming('on');
              }}
            >
              {t('integrations.switchOn')}
            </Button>
          )}
        </div>
      )}

      {secretNames.length > 0 && (
        <>
          <h4 className={styles.heading}>{t('integrations.secrets')}</h4>
          <p className={styles.muted}>{t('integrations.secretsNote')}</p>
          {secretNames.map((name) => (
            <SecretRow
              key={name}
              name={name}
              storedAt={connection.secrets[name]}
              busy={busy}
              onSave={(value) => {
                run(() => setMcpSecret(connection.id, name, value));
              }}
              onClear={() => {
                run(() => clearMcpSecret(connection.id, name));
              }}
            />
          ))}
        </>
      )}

      <h4 className={styles.heading}>{t('integrations.examine')}</h4>
      <p className={styles.muted}>
        {t('integrations.examineNote')}
        {FETCHES_CODE.has(program) && ` ${t('integrations.examineDownloads')}`}
      </p>
      <ExamineRow
        connection={connection}
        runtimes={usable}
        disabled={busy || !connection.enabled || missing.length > 0}
        onExamine={(runtimeId) => {
          run(() => probeMcpConnection(connection.id, runtimeId));
        }}
      />

      <h4 className={styles.heading}>{t('integrations.grants')}</h4>
      {mine.length === 0 && <p className={styles.muted}>{t('integrations.grantsNone')}</p>}
      <ul className={styles.list}>
        {mine.map((grant) => (
          <li key={grant.id} className={styles.row}>
            <span>
              {agents.find((a) => a.id === grant.agentId)?.name ?? t('integrations.anyAgent')}
              {' — '}
              {grant.tools.kind === 'server'
                ? t('integrations.toolsAll')
                : t('integrations.toolsOnly', { tools: grant.tools.tools.join(', ') })}
            </span>
            <button
              type="button"
              className={styles.linkButton}
              disabled={busy}
              onClick={() => {
                run(() => revokeMcpGrant(grant.id));
              }}
            >
              {t('integrations.revoke')}
            </button>
          </li>
        ))}
      </ul>
      <GrantForm
        connection={connection}
        agents={agents}
        disabled={busy}
        onGrant={(agentId, tools) => {
          run(() => grantMcpConnection(connection.id, agentId, tools));
        }}
      />

      {confirming === 'remove' ? (
        <div className={styles.row} role="group" aria-label={t('integrations.removeConfirm')}>
          <Button
            variant="danger"
            disabled={busy}
            onClick={() => {
              run(() => removeMcpConnection(connection.id));
            }}
          >
            {t('integrations.removeConfirm')}
          </Button>
          <button
            type="button"
            className={styles.linkButton}
            onClick={() => {
              setConfirming(null);
            }}
          >
            {t('common.cancel')}
          </button>
        </div>
      ) : (
        <div className={styles.row}>
          <button
            type="button"
            className={styles.linkButton}
            onClick={() => {
              setConfirming('remove');
            }}
          >
            {t('integrations.remove')}
          </button>
        </div>
      )}
    </li>
  );
}

function SecretRow({
  name,
  storedAt,
  busy,
  onSave,
  onClear,
}: {
  name: string;
  storedAt: number | undefined;
  busy: boolean;
  onSave: (value: string) => void;
  onClear: () => void;
}) {
  const { t } = useI18n();
  const [value, setValue] = useState('');
  return (
    <div className={styles.row}>
      <code className={styles.mono}>{name}</code>
      <span className={styles.muted}>
        {storedAt === undefined ? t('integrations.secretMissing') : t('integrations.secretStored')}
      </span>
      <input
        type="password"
        autoComplete="off"
        aria-label={t('integrations.secretInput', { name })}
        value={value}
        onChange={(e) => {
          setValue(e.target.value);
        }}
      />
      <Button
        variant="secondary"
        disabled={busy || value === ''}
        onClick={() => {
          onSave(value);
          // Not kept in the page a moment longer than it takes to hand over.
          setValue('');
        }}
      >
        {t('integrations.secretSave')}
      </Button>
      {storedAt !== undefined && (
        <button type="button" className={styles.linkButton} disabled={busy} onClick={onClear}>
          {t('integrations.secretClear')}
        </button>
      )}
    </div>
  );
}

function ExamineRow({
  connection,
  runtimes,
  disabled,
  onExamine,
}: {
  connection: McpConnectionDto;
  runtimes: RuntimeStatus[];
  disabled: boolean;
  onExamine: (runtimeId: string) => void;
}) {
  const { t } = useI18n();
  const [chosen, setChosen] = useState('');
  const runtimeId = chosen !== '' ? chosen : (runtimes[0]?.runtime.id ?? '');
  const found = connection.discovery;
  return (
    <>
      {runtimes.length === 0 ? (
        <p className={styles.muted}>{t('integrations.noRuntime')}</p>
      ) : (
        <div className={styles.row}>
          <select
            aria-label={t('integrations.examineWith')}
            value={runtimeId}
            onChange={(e) => {
              setChosen(e.target.value);
            }}
          >
            {runtimes.map((r) => (
              <option key={r.runtime.id} value={r.runtime.id}>
                {r.runtime.name}
              </option>
            ))}
          </select>
          <Button
            variant="secondary"
            disabled={disabled || runtimeId === ''}
            onClick={() => {
              onExamine(runtimeId);
            }}
          >
            {t('integrations.examineAction')}
          </Button>
        </div>
      )}
      {found && (
        <p className={styles.muted}>
          {t(`context.mcp.statusName.${found.status}` as TranslationKey)} —{' '}
          {found.tools.length === 0
            ? t('integrations.noTools')
            : t('integrations.tools', { tools: found.tools.join(', ') })}
        </p>
      )}
    </>
  );
}

function GrantForm({
  connection,
  agents,
  disabled,
  onGrant,
}: {
  connection: McpConnectionDto;
  agents: Agent[];
  disabled: boolean;
  onGrant: (agentId: string, tools: { kind: 'server' } | { kind: 'only'; tools: string[] }) => void;
}) {
  const { t } = useI18n();
  const [agentId, setAgentId] = useState('');
  const [mode, setMode] = useState<'server' | 'only'>('server');
  const [picked, setPicked] = useState<ReadonlySet<string>>(new Set());
  const discovered = connection.discovery?.tools ?? [];
  const agent = agentId !== '' ? agentId : (agents[0]?.id ?? '');
  if (agents.length === 0) return null;
  const ready = agent !== '' && (mode === 'server' || picked.size > 0);
  return (
    <div className={styles.field}>
      <label>
        {t('integrations.grantAgent')}{' '}
        <select
          value={agent}
          onChange={(e) => {
            setAgentId(e.target.value);
          }}
        >
          {agents.map((a) => (
            <option key={a.id} value={a.id}>
              {a.name}
            </option>
          ))}
        </select>
      </label>
      <label>
        <input
          type="radio"
          name={`mode-${connection.id}`}
          checked={mode === 'server'}
          onChange={() => {
            setMode('server');
          }}
        />{' '}
        {t('integrations.grantAll')}
      </label>
      <label>
        <input
          type="radio"
          name={`mode-${connection.id}`}
          checked={mode === 'only'}
          disabled={discovered.length === 0}
          onChange={() => {
            setMode('only');
          }}
        />{' '}
        {t('integrations.grantOnly')}
      </label>
      {discovered.length === 0 && (
        <p className={styles.muted}>{t('integrations.grantNeedsExamine')}</p>
      )}
      {mode === 'only' &&
        discovered.map((tool) => (
          <label key={tool}>
            <input
              type="checkbox"
              checked={picked.has(tool)}
              onChange={(e) => {
                setPicked((current) => {
                  const next = new Set(current);
                  if (e.target.checked) next.add(tool);
                  else next.delete(tool);
                  return next;
                });
              }}
            />{' '}
            <code className={styles.mono}>{tool}</code>
          </label>
        ))}
      <div className={styles.row}>
        <Button
          variant="secondary"
          disabled={disabled || !ready}
          onClick={() => {
            onGrant(
              agent,
              mode === 'server' ? { kind: 'server' } : { kind: 'only', tools: [...picked] },
            );
          }}
        >
          {t('integrations.grantAction')}
        </Button>
      </div>
    </div>
  );
}
