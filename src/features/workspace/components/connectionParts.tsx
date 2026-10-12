import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import type { Agent, RuntimeStatus } from '@/features/agents/types';
import type { TranslationKey } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import type { McpConnectionDto, McpGrantDto, WorkflowDto } from '@/lib/tauri/commands';
import styles from './Integrations.module.css';

/** Programs that fetch and run code from the network the first time they start. */
export const FETCHES_CODE = new Set(['npx', 'uvx', 'bunx', 'pnpx', 'dlx']);

/**
 * How the runtime an agent uses gets a server: `handed` (Atlas gives it for the run), `cli` (it
 * only uses the servers of its own configuration; Atlas checks the server is there and tells the
 * agent) or `none` (it cannot be given any).
 */
export type Delivery = 'handed' | 'cli' | 'none';

export function deliveryTo(agent: Agent | undefined, runtimes: RuntimeStatus[]): Delivery {
  if (!agent) return 'handed';
  const support = runtimes.find((r) => r.runtime.id === agent.runtimeId)?.runtime.capabilities.mcp;
  if (support === 'supported') return 'handed';
  return support === 'cli_configured' ? 'cli' : 'none';
}

/** Whether the runtime an agent uses can be given MCP servers by Atlas. */
export function canReceive(agent: Agent | undefined, runtimes: RuntimeStatus[]): boolean {
  return deliveryTo(agent, runtimes) === 'handed';
}

/** Where a grant applies, in words: a workflow, and a step of it when it names one. */
export function scopeLabel(
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

export function SecretRow({
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

export function ExamineRow({
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

export function GrantForm({
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
      {deliveryTo(
        agents.find((a) => a.id === agent),
        runtimes,
      ) === 'none' && (
        <p className={styles.warning} role="alert">
          {t('integrations.runtimeCannotLong')}
        </p>
      )}
      {deliveryTo(
        agents.find((a) => a.id === agent),
        runtimes,
      ) === 'cli' && (
        <p className={styles.warning} role="note">
          {t('integrations.viaCliLong')}
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
