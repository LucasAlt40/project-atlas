import { useEffect, useState } from 'react';
import { Badge } from '@/components/ui/Badge';
import { Callout } from '@/components/ui/Callout';
import { listMcpConnections } from '@/features/workspace/services/mcpService';
import { useI18n } from '@/i18n/I18nProvider';
import type { McpConnectionDto } from '@/lib/tauri/commands';
import styles from './Specs.module.css';

interface Props {
  workspaceId: string;
  selected: string[];
  onChange: (ids: string[]) => void;
  /** Choose every connection that is switched on, once they are known. */
  preselect?: boolean;
}

/**
 * The workspace's integrations (MCP connections), to choose the ones this spec is tested and
 * reviewed with. Choosing grants nothing: the MCP hub decides what a step is given.
 */
export function IntegrationsPicker({ workspaceId, selected, onChange, preselect = false }: Props) {
  const { t } = useI18n();
  const [connections, setConnections] = useState<McpConnectionDto[] | null>(null);

  useEffect(() => {
    let current = true;
    Promise.resolve(listMcpConnections(workspaceId))
      .then((overview) => {
        if (!current) return;
        setConnections(overview.connections);
        if (preselect) {
          onChange(overview.connections.filter((c) => c.enabled).map((c) => c.id));
        }
      })
      .catch(() => {
        if (current) setConnections([]);
      });
    return () => {
      current = false;
    };
    // Read once per workspace: the choice is the person's after that.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [workspaceId]);

  if (connections === null) return null;
  const toggle = (id: string, on: boolean) => {
    onChange(on ? [...selected, id] : selected.filter((chosen) => chosen !== id));
  };
  return (
    <section className={styles.tools} aria-labelledby="spec-tools-title">
      <div className={styles.toolsHead}>
        <h3 id="spec-tools-title" className={styles.toolsTitle}>
          {t('specs.integrations.title')}
        </h3>
        {connections.length > 0 && (
          <Badge mono>
            {String(selected.length)} / {String(connections.length)}
          </Badge>
        )}
      </div>
      <p className={styles.muted}>{t('specs.integrations.intro')}</p>
      {connections.length === 0 ? (
        <p className={styles.hint}>{t('specs.integrations.none')}</p>
      ) : (
        <ul className={styles.toolList}>
          {connections.map((connection) => {
            const on = selected.includes(connection.id);
            return (
              <li key={connection.id}>
                <label className={styles.toolRow} data-on={on} data-off={!connection.enabled}>
                  <input
                    type="checkbox"
                    aria-label={connection.name}
                    checked={on}
                    disabled={!connection.enabled}
                    onChange={(event) => {
                      toggle(connection.id, event.target.checked);
                    }}
                  />
                  <span className={styles.toolName}>{connection.name}</span>
                  {connection.enabled ? (
                    <Badge dot tone="success">
                      {connection.discovery
                        ? t('specs.integrations.tools', {
                            count: connection.discovery.tools.length,
                          })
                        : t('specs.integrations.notListed')}
                    </Badge>
                  ) : (
                    <Badge dot>{t('specs.integrations.off')}</Badge>
                  )}
                </label>
              </li>
            );
          })}
        </ul>
      )}
      <Callout tone="policy">{t('specs.integrations.grants')}</Callout>
    </section>
  );
}
