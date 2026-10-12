import { useCallback, useEffect, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { useI18n } from '@/i18n/I18nProvider';
import type { McpOverviewDto, McpToolSelectionDto } from '@/lib/tauri/commands';
import {
  grantMcpConnection,
  listMcpConnections,
  revokeMcpGrant,
} from '@/features/workspace/services/mcpService';
import { readsOnly } from '../model/toolNames';
import type { AgentNode, Workflow } from '../types';
import styles from './Workflow.module.css';

interface Props {
  workspaceId: string;
  workflow: Workflow;
  node: AgentNode;
  /** The step is in the stored workflow: a grant can only name a step the core knows. */
  saved: boolean;
}

type Scope = 'all' | 'reading';

/**
 * The tools (MCP connections) one step may use: what is granted to it, and a way to grant or
 * revoke. The grants are the MCP platform's own (agent, workflow and step), the same ones the hub
 * shows; this is another door to them, not another copy.
 */
export function StepTools({ workspaceId, workflow, node, saved }: Props) {
  const { t } = useI18n();
  const [overview, setOverview] = useState<McpOverviewDto | null>(null);
  const [connectionId, setConnectionId] = useState('');
  const [scope, setScope] = useState<Scope>('all');
  const [failed, setFailed] = useState(false);

  const load = useCallback(() => {
    listMcpConnections(workspaceId)
      .then((value) => {
        setOverview(value);
        setFailed(false);
      })
      .catch(() => {
        setFailed(true);
      });
  }, [workspaceId]);
  useEffect(() => {
    load();
  }, [load]);

  if (!overview) return failed ? null : <p className={styles.muted}>…</p>;

  const mine = overview.grants.filter(
    (grant) => grant.workflowId === workflow.id && grant.nodeId === node.id,
  );
  const name = (id: string) => overview.connections.find((c) => c.id === id)?.name ?? id;
  const usable = overview.connections.filter((c) => c.enabled);
  const chosen = usable.find((c) => c.id === connectionId);
  const known = chosen?.discovery?.tools ?? null;
  const reading = known?.filter(readsOnly) ?? [];

  const describe = (tools: McpToolSelectionDto) =>
    tools.kind === 'only'
      ? t('workflow.inspector.toolsOnly', { tools: tools.tools.join(', ') })
      : t('workflow.inspector.toolsAll');

  const grant = () => {
    if (!chosen || node.agentId === '') return;
    const tools: McpToolSelectionDto =
      scope === 'reading' ? { kind: 'only', tools: reading } : { kind: 'server' };
    grantMcpConnection(chosen.id, node.agentId, tools, {
      workflowId: workflow.id,
      nodeId: node.id,
    })
      .then(() => {
        setConnectionId('');
        load();
      })
      .catch(() => {
        setFailed(true);
      });
  };

  return (
    <div className={styles.stepTools}>
      <div className={styles.toolsHead}>
        <span className={styles.fieldLabel}>{t('workflow.inspector.tools')}</span>
        <span className={styles.muted}>
          {t('workflow.inspector.toolsActive', { n: mine.length })}
        </span>
      </div>
      {node.executionPolicy.readOnly && mine.length > 0 && (
        <p className={styles.muted}>{t('workflow.inspector.toolsReadOnlyWarning')}</p>
      )}
      {mine.length === 0 ? (
        <p className={styles.muted}>{t('workflow.inspector.toolsNone')}</p>
      ) : (
        <ul>
          {mine.map((grant) => (
            <li key={grant.id} className={styles.toolRow}>
              <Icon name="plug" size={15} />
              <span className={styles.toolRowText}>
                <strong>{name(grant.connectionId)}</strong>
                <span className={styles.muted}>{describe(grant.tools)}</span>
              </span>
              <button
                type="button"
                className={styles.iconButton}
                aria-label={t('workflow.inspector.toolsRevoke', {
                  name: name(grant.connectionId),
                })}
                title={t('workflow.inspector.toolsRevoke', { name: name(grant.connectionId) })}
                onClick={() => {
                  revokeMcpGrant(grant.id).then(load, () => {
                    setFailed(true);
                  });
                }}
              >
                <Icon name="close" size={14} />
              </button>
            </li>
          ))}
        </ul>
      )}
      {node.agentId === '' ? (
        <p className={styles.muted}>{t('workflow.inspector.toolsNoAgent')}</p>
      ) : !saved ? (
        <p className={styles.muted}>{t('workflow.inspector.toolsUnsaved')}</p>
      ) : usable.length === 0 ? (
        <p className={styles.muted}>{t('workflow.inspector.toolsNoConnections')}</p>
      ) : (
        <>
          <label className={styles.field}>
            <span className={styles.fieldLabel}>{t('workflow.inspector.toolsConnection')}</span>
            <select
              className={styles.control}
              value={connectionId}
              onChange={(e) => {
                setConnectionId(e.target.value);
                setScope('all');
              }}
            >
              <option value="">{t('workflow.inspector.toolsChoose')}</option>
              {usable.map((connection) => (
                <option key={connection.id} value={connection.id}>
                  {connection.name}
                </option>
              ))}
            </select>
          </label>
          {chosen && (
            <label className={styles.field}>
              <span className={styles.fieldLabel}>{t('workflow.inspector.toolsScope')}</span>
              <select
                className={styles.control}
                value={scope}
                onChange={(e) => {
                  setScope(e.target.value as Scope);
                }}
              >
                <option value="all">{t('workflow.inspector.toolsScopeAll')}</option>
                <option value="reading" disabled={reading.length === 0}>
                  {t('workflow.inspector.toolsScopeReading')}
                </option>
              </select>
            </label>
          )}
          <Button type="button" disabled={!chosen} onClick={grant}>
            {t('workflow.inspector.toolsGrant')}
          </Button>
        </>
      )}
    </div>
  );
}
