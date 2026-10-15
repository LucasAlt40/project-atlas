import { useEffect, useState } from 'react';
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
  return (
    <fieldset className={styles.integrations}>
      <legend>{t('specs.integrations.title')}</legend>
      <p className={styles.muted}>{t('specs.integrations.intro')}</p>
      {connections.length === 0 ? (
        <p className={styles.hint}>{t('specs.integrations.none')}</p>
      ) : (
        <ul className={styles.plain}>
          {connections.map((connection) => (
            <li key={connection.id}>
              <label className={styles.inline}>
                <input
                  type="checkbox"
                  checked={selected.includes(connection.id)}
                  disabled={!connection.enabled}
                  onChange={(event) => {
                    onChange(
                      event.target.checked
                        ? [...selected, connection.id]
                        : selected.filter((id) => id !== connection.id),
                    );
                  }}
                />
                <span>{connection.name}</span>
              </label>
              <span className={styles.muted}>
                {connection.enabled
                  ? connection.discovery
                    ? t('specs.integrations.tools', { count: connection.discovery.tools.length })
                    : t('specs.integrations.notListed')
                  : t('specs.integrations.off')}
              </span>
            </li>
          ))}
        </ul>
      )}
      <p className={styles.hint}>{t('specs.integrations.grants')}</p>
    </fieldset>
  );
}
