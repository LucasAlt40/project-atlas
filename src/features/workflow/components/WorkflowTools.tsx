import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { listMcpConnections } from '@/features/workspace/services/mcpService';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type { McpOverviewDto } from '@/lib/tauri/commands';
import { connectionViews } from '../model/toolsView';
import type { Workflow } from '../types';
import styles from './Workflow.module.css';

interface Props {
  workspaceId: string;
  workflow: Workflow;
  onOpenHub: () => void;
}

/**
 * The workspace's MCP connections as this workflow sees them: the state each one is really in
 * (never assumed), whether it is switched on for the workspace, and which steps of this workflow
 * it is granted to. Changing a grant is done in the step; adding or switching on a connection,
 * in the MCP hub.
 */
export function WorkflowTools({ workspaceId, workflow, onOpenHub }: Props) {
  const { t } = useI18n();
  const [overview, setOverview] = useState<McpOverviewDto | 'failed' | null>(null);
  useEffect(() => {
    let cancelled = false;
    listMcpConnections(workspaceId)
      .then((value) => {
        if (!cancelled) setOverview(value);
      })
      .catch(() => {
        if (!cancelled) setOverview('failed');
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceId, workflow]);

  return (
    <section className={styles.card} aria-label={t('workflow.tools')}>
      <h4>
        <Icon name="plug" size={14} />
        {t('workflow.tools')}
      </h4>
      {overview === null && <p className={styles.muted}>…</p>}
      {overview === 'failed' && <p className={styles.muted}>{t('workflow.tools.unavailable')}</p>}
      {overview !== null && overview !== 'failed' && overview.connections.length === 0 && (
        <p className={styles.muted}>{t('workflow.tools.none')}</p>
      )}
      {overview !== null && overview !== 'failed' && (
        <ul>
          {connectionViews(workflow, overview.connections, overview.grants).map((view) => (
            <li key={view.connection.id}>
              <span className={styles.stepName}>
                <span
                  className={styles.stepDot}
                  data-status={
                    !view.connection.enabled
                      ? 'pending'
                      : view.status === 'connected'
                        ? 'completed'
                        : view.status === 'failed' || view.status === 'needs_auth'
                          ? 'failed'
                          : 'pending'
                  }
                  aria-hidden="true"
                >
                  {''}
                </span>
                <strong>{view.connection.name}</strong>
              </span>
              <span className={styles.stepState}>
                {view.connection.enabled ? t('workflow.tools.on') : t('workflow.tools.off')}
                {' · '}
                {view.status
                  ? t(`context.mcp.statusName.${view.status}` as TranslationKey)
                  : t('workflow.tools.neverChecked')}
              </span>
              <span className={styles.muted}>
                {view.steps.length > 0
                  ? view.steps
                      .map((step) =>
                        step.tools.kind === 'only'
                          ? `${step.label} (${step.tools.tools.join(', ')})`
                          : step.label,
                      )
                      .join(' · ')
                  : view.broad > 0
                    ? t('workflow.tools.broad')
                    : t('workflow.tools.noSteps')}
              </span>
            </li>
          ))}
        </ul>
      )}
      <Button type="button" variant="secondary" onClick={onOpenHub}>
        {t('workflow.tools.hub')}
      </Button>
    </section>
  );
}
