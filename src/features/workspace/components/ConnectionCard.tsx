import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import type { Agent, RuntimeStatus } from '@/features/agents/types';
import type { TranslationKey } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import { errorDetail, errorMessage } from '@/i18n/messages';
import type { McpConnectionDto, McpGrantDto, WorkflowDto } from '@/lib/tauri/commands';
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
  workflows: WorkflowDto[];
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
  workflows,
  riskNoteKey,
  onChanged,
}: Props) {
  const { t } = useI18n();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [detail, setDetail] = useState<string | undefined>();
  const [probing, setProbing] = useState(false);
  const [confirming, setConfirming] = useState<'on' | 'remove' | null>(null);

  function run(action: () => Promise<unknown>) {
    setBusy(true);
    setError(null);
    setDetail(undefined);
    action()
      .then(() => {
        setConfirming(null);
        onChanged();
      })
      .catch((e: unknown) => {
        setError(errorMessage(t, e));
        setDetail(errorDetail(e));
      })
      .finally(() => {
        setBusy(false);
        setProbing(false);
      });
  }

  const usable = runtimes.filter(
    (r) =>
      r.runtime.capabilities.mcp === 'supported' &&
      r.runtime.capabilities.mcpFeatures.probe !== 'none',
  );
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
          {detail && <code className={styles.mono}> {detail}</code>}
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
          setProbing(true);
          run(() => probeMcpConnection(connection.id, runtimeId));
        }}
      />
      {probing && (
        <p className={styles.muted} role="status">
          {t('integrations.examining')}
        </p>
      )}

      <h4 className={styles.heading}>{t('integrations.grants')}</h4>
      <p className={styles.muted}>{t('integrations.grantsNote')}</p>
      {mine.length === 0 && <p className={styles.muted}>{t('integrations.grantsNone')}</p>}
      <ul className={styles.list}>
        {mine.map((grant) => (
          <li key={grant.id} className={styles.row}>
            <span>
              {agents.find((a) => a.id === grant.agentId)?.name ?? t('integrations.anyAgent')}
              {grant.workflowId !== null && ` · ${scopeLabel(grant, workflows, t)}`}
              {' — '}
              {grant.tools.kind === 'server'
                ? t('integrations.toolsAll')
                : t('integrations.toolsOnly', { tools: grant.tools.tools.join(', ') })}
              {!canReceive(
                agents.find((a) => a.id === grant.agentId),
                runtimes,
              ) && <span className={styles.error}> ⚠ {t('integrations.runtimeCannot')}</span>}
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
        runtimes={runtimes}
        workflows={workflows}
        disabled={busy}
        onGrant={(agentId, tools, scope) => {
          run(() => grantMcpConnection(connection.id, agentId, tools, scope));
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

/** Whether the runtime an agent uses can be given MCP servers (only Claude, today). */
function canReceive(agent: Agent | undefined, runtimes: RuntimeStatus[]): boolean {
  if (!agent) return true;
  return (
    runtimes.find((r) => r.runtime.id === agent.runtimeId)?.runtime.capabilities.mcp === 'supported'
  );
}

/** Where a grant applies, in words: a workflow, and a step of it when it names one. */
function scopeLabel(
  grant: McpGrantDto,
  workflows: WorkflowDto[],
  t: (key: TranslationKey, params?: Record<string, string | number>) => string,
): string {
  const workflow = workflows.find((w) => w.id === grant.workflowId);
  const name = workflow?.name ?? t('integrations.scopeUnknown');
  if (grant.nodeId === null) return t('integrations.scopeWorkflow', { workflow: name });
  const node = workflow?.nodes.find((n) => n.id === grant.nodeId);
  const step = node && 'label' in node ? node.label : grant.nodeId;
  return t('integrations.scopeStep', { workflow: name, step });
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
  runtimes,
  workflows,
  disabled,
  onGrant,
}: {
  connection: McpConnectionDto;
  agents: Agent[];
  runtimes: RuntimeStatus[];
  workflows: WorkflowDto[];
  disabled: boolean;
  onGrant: (
    agentId: string,
    tools: { kind: 'server' } | { kind: 'only'; tools: string[] },
    scope: { workflowId?: string; nodeId?: string },
  ) => void;
}) {
  const { t } = useI18n();
  const [agentId, setAgentId] = useState('');
  const [mode, setMode] = useState<'server' | 'only'>('server');
  const [picked, setPicked] = useState<ReadonlySet<string>>(new Set());
  const [workflowId, setWorkflowId] = useState('');
  const [nodeId, setNodeId] = useState('');
  const [typed, setTyped] = useState('');
  const discovered = connection.discovery?.tools ?? [];
  const workflow = workflows.find((w) => w.id === workflowId);
  // Only a step that runs an agent can use a tool, and the grant is for the agent it runs.
  const steps = (workflow?.nodes ?? []).flatMap((n) => (n.type === 'agent' ? [n] : []));
  const step = steps.find((n) => n.id === nodeId);
  const stepAgent = step?.agentId;
  const agent = stepAgent ?? (agentId !== '' ? agentId : (agents[0]?.id ?? ''));
  if (agents.length === 0) return null;
  // What this agent's runtime can do to hold a server to the tools named.
  const features = runtimes.find(
    (r) => r.runtime.id === agents.find((a) => a.id === agent)?.runtimeId,
  )?.runtime.capabilities.mcpFeatures;
  const filter = features?.toolFilter ?? 'unsupported';
  // An allow-list needs no discovery: the names can be typed. A deny-list needs the tools known.
  const manual = filter === 'allow_list' && discovered.length === 0;
  const typedTools = typed
    .split(/[\s,]+/)
    .map((name) => name.trim())
    .filter((name) => name !== '');
  const named = [...new Set([...picked, ...(manual ? typedTools : [])])];
  const canName = filter !== 'unsupported' && (discovered.length > 0 || manual);
  const ready = agent !== '' && (mode === 'server' || (canName && named.length > 0));
  return (
    <div className={styles.field}>
      {!canReceive(
        agents.find((a) => a.id === agent),
        runtimes,
      ) && (
        <p className={styles.warning} role="alert">
          {t('integrations.runtimeCannotLong')}
        </p>
      )}
      <label>
        {t('integrations.grantAgent')}{' '}
        <select
          value={agent}
          disabled={stepAgent !== undefined}
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
      {workflows.length > 0 && (
        <>
          <label>
            {t('integrations.grantWorkflow')}{' '}
            <select
              value={workflowId}
              onChange={(e) => {
                setWorkflowId(e.target.value);
                setNodeId('');
              }}
            >
              <option value="">{t('integrations.grantAnyRun')}</option>
              {workflows.map((w) => (
                <option key={w.id} value={w.id}>
                  {w.name}
                </option>
              ))}
            </select>
          </label>
          {workflow && (
            <label>
              {t('integrations.grantStep')}{' '}
              <select
                value={nodeId}
                onChange={(e) => {
                  setNodeId(e.target.value);
                }}
              >
                <option value="">{t('integrations.grantAnyStep')}</option>
                {steps.map((n) => (
                  <option key={n.id} value={n.id}>
                    {n.label}
                  </option>
                ))}
              </select>
            </label>
          )}
        </>
      )}
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
          disabled={!canName}
          onChange={() => {
            setMode('only');
          }}
        />{' '}
        {t('integrations.grantOnly')}
      </label>
      {filter === 'unsupported' && (
        <p className={styles.muted}>{t('integrations.filterUnsupported')}</p>
      )}
      {filter === 'deny_list' && discovered.length === 0 && (
        <p className={styles.muted}>{t('integrations.grantNeedsExamine')}</p>
      )}
      {features && !features.strict && (
        <p className={styles.muted}>{t('integrations.notStrict')}</p>
      )}
      {mode === 'only' && manual && (
        <label className={styles.field}>
          {t('integrations.typeTools')}
          <input
            value={typed}
            onChange={(e) => {
              setTyped(e.target.value);
            }}
          />
        </label>
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
              mode === 'server' ? { kind: 'server' } : { kind: 'only', tools: named },
              {
                ...(workflowId !== '' && { workflowId }),
                ...(workflowId !== '' && nodeId !== '' && { nodeId }),
              },
            );
          }}
        >
          {t('integrations.grantAction')}
        </Button>
      </div>
    </div>
  );
}
