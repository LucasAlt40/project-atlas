import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import type { Agent, RuntimeStatus } from '@/features/agents/types';
import { useI18n } from '@/i18n/I18nProvider';
import type { McpConnectionDto, McpGrantDto, WorkflowDto } from '@/lib/tauri/commands';
import { grantMcpConnection, revokeMcpGrant } from '../../services/mcpService';
import { GrantForm, deliveryTo, scopeLabel } from '../connectionParts';
import styles from './McpHub.module.css';

interface Props {
  connection: McpConnectionDto;
  grants: McpGrantDto[];
  agents: Agent[];
  runtimes: RuntimeStatus[];
  workflows: WorkflowDto[];
  busy: boolean;
  run: (action: () => Promise<unknown>) => void;
}

/** A grant of the whole server to one agent, for any run: what a checkbox stands for. */
const isPlain = (g: McpGrantDto) =>
  g.agentId !== null && g.workflowId === null && g.nodeId === null && g.tools.kind === 'server';

/**
 * Who may use a server, as a list to tick: the agents that have it are ticked, ticking and
 * unticking changes nothing until "Apply", and one click then grants and revokes what changed.
 * Narrower grants (one workflow or step, named tools) stay listed apart, and the form for making
 * them is one fold away.
 */
export function GrantAgents({ connection, grants, agents, runtimes, workflows, busy, run }: Props) {
  const { t } = useI18n();
  const mine = grants.filter((g) => g.connectionId === connection.id);
  const plain = mine.filter(isPlain);
  const granted = new Set(plain.map((g) => g.agentId));
  const specific = mine.filter((g) => !isPlain(g));
  // Starts as what is granted; the parent re-keys this when the grants change.
  const [draft, setDraft] = useState<ReadonlySet<string | null>>(granted);
  const add = agents.filter((a) => draft.has(a.id) && !granted.has(a.id));
  const remove = agents.filter((a) => !draft.has(a.id) && granted.has(a.id));
  const changes = add.length + remove.length;

  function toggle(id: string, on: boolean) {
    setDraft((current) => {
      const next = new Set(current);
      if (on) next.add(id);
      else next.delete(id);
      return next;
    });
  }

  function apply() {
    run(async () => {
      for (const agent of add) {
        await grantMcpConnection(connection.id, agent.id, { kind: 'server' }, {});
      }
      for (const agent of remove) {
        for (const grant of plain.filter((g) => g.agentId === agent.id)) {
          await revokeMcpGrant(grant.id);
        }
      }
    });
  }

  const runtimeName = (agent: Agent) =>
    runtimes.find((r) => r.runtime.id === agent.runtimeId)?.runtime.name ?? agent.runtimeId;

  return (
    <div className={styles.stack}>
      {agents.length === 0 && <p className={styles.muted}>{t('hub.grant.noAgents')}</p>}
      <ul className={styles.agentList} aria-label={t('integrations.grants')}>
        {agents.map((agent) => {
          const delivery = deliveryTo(agent, runtimes);
          return (
            <li key={agent.id}>
              <label
                className={styles.agentRow}
                data-changed={draft.has(agent.id) !== granted.has(agent.id)}
              >
                <input
                  type="checkbox"
                  checked={draft.has(agent.id)}
                  disabled={busy}
                  onChange={(e) => {
                    toggle(agent.id, e.target.checked);
                  }}
                />
                <span className={styles.agentName}>{agent.name}</span>
                <span className={styles.chip}>{runtimeName(agent)}</span>
                {delivery === 'none' && (
                  <span
                    className={styles.chip}
                    data-tone="bad"
                    title={t('integrations.runtimeCannot')}
                  >
                    {t('hub.grant.noDelivery')}
                  </span>
                )}
                {delivery === 'cli' && (
                  <span className={styles.chip} data-tone="warn" title={t('integrations.viaCli')}>
                    {t('hub.grant.viaCli')}
                  </span>
                )}
              </label>
            </li>
          );
        })}
      </ul>
      <div className={styles.row}>
        <Button disabled={busy || changes === 0} onClick={apply}>
          {changes === 0 ? t('hub.grant.apply') : t('hub.grant.applyN', { n: changes })}
        </Button>
        <button
          type="button"
          className={styles.link}
          disabled={busy}
          onClick={() => {
            setDraft(new Set(agents.map((a) => a.id)));
          }}
        >
          {t('hub.grant.all')}
        </button>
        <button
          type="button"
          className={styles.link}
          disabled={busy}
          onClick={() => {
            setDraft(new Set());
          }}
        >
          {t('hub.grant.none')}
        </button>
      </div>

      {specific.length > 0 && (
        <div className={styles.stack}>
          <span className={styles.eyebrow}>{t('hub.grant.specific')}</span>
          <ul className={styles.plainList}>
            {specific.map((grant) => (
              <li key={grant.id} className={styles.rowBetween}>
                <span className={styles.muted}>
                  {agents.find((a) => a.id === grant.agentId)?.name ?? t('integrations.anyAgent')}
                  {grant.workflowId !== null && ` · ${scopeLabel(grant, workflows, t)}`}
                  {' — '}
                  {grant.tools.kind === 'server'
                    ? t('integrations.toolsAll')
                    : t('integrations.toolsOnly', { tools: grant.tools.tools.join(', ') })}
                </span>
                <button
                  type="button"
                  className={styles.link}
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
        </div>
      )}

      <details className={styles.fold}>
        <summary>{t('hub.grant.advanced')}</summary>
        <GrantForm
          connection={connection}
          agents={agents}
          runtimes={runtimes}
          workflows={workflows}
          disabled={busy}
          onGrant={(agentId, selection, scope) => {
            run(() => grantMcpConnection(connection.id, agentId, selection, scope));
          }}
        />
      </details>
    </div>
  );
}
