import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type { McpProblemDto, McpRecordDto } from '@/lib/tauri/commands';
import styles from './ContextReview.module.css';

/** A flag the runtime may not have reported: `null` is "no report", which is not "no". */
function Mark({ value, label }: { value: boolean | null; label: string }) {
  const { t } = useI18n();
  const text =
    value === null ? t('context.mcp.unknown') : value ? t('context.mcp.yes') : t('context.mcp.no');
  return (
    <td aria-label={`${label}: ${text}`} data-value={String(value)}>
      <span aria-hidden="true">{value === null ? '?' : value ? '✓' : '–'}</span>
    </td>
  );
}

function problemKey(problem: McpProblemDto): TranslationKey {
  return `context.mcp.problem.${problem.kind}` as TranslationKey;
}

/**
 * The workspace's MCP connections as one step saw them. The point of the table is that a server
 * being connected, a tool being discovered, enabled, authorized, exposed to the runtime and used
 * are six different facts: each is its own column, and "?" is a runtime that did not say.
 */
export function McpPanel({ record }: { record: McpRecordDto }) {
  const { t } = useI18n();
  const columns = ['discovered', 'enabled', 'authorized', 'exposed', 'reported', 'used'] as const;

  return (
    <section className={styles.panel} aria-label={t('context.mcp.title')}>
      <h3>{t('context.mcp.title').toUpperCase()}</h3>
      <p className={styles.line}>{t('context.mcp.note')}</p>
      {record.unauthorized.length > 0 && (
        <p className={styles.status} data-health="invalid" role="alert">
          ✕ {t('context.mcp.unauthorized', { tools: record.unauthorized.join(', ') })}
        </p>
      )}
      {record.servers.map((server) => {
        const tools = record.tools.filter((tool) => tool.server === server.name);
        return (
          <div key={server.connectionId}>
            <h4 className={styles.heading}>{server.name}</h4>
            <p className={styles.line}>
              {[
                server.enabled ? t('context.mcp.enabled') : t('context.mcp.disabled'),
                server.required ? t('context.mcp.required') : t('context.mcp.optional'),
                server.authorized ? t('context.mcp.authorized') : t('context.mcp.notAuthorized'),
                server.exposed ? t('context.mcp.exposed') : t('context.mcp.notExposed'),
              ].join(' · ')}
            </p>
            {server.problem && (
              <p className={styles.line}>
                <span className={styles.severity} data-severity="warning">
                  ⚠ {t(problemKey(server.problem))}
                </span>
                {server.problem.kind === 'secret_missing' && (
                  <>
                    {' '}
                    <code className={styles.mono}>{server.problem.name}</code>
                  </>
                )}
                {server.problem.kind === 'invalid_configuration' && (
                  <>
                    {' '}
                    <code className={styles.mono}>{server.problem.reason}</code>
                  </>
                )}
              </p>
            )}
            <p className={styles.line}>
              {t('context.mcp.status', {
                discovered: server.discoveredStatus
                  ? t(`context.mcp.statusName.${server.discoveredStatus}` as TranslationKey)
                  : t('context.mcp.never'),
                reported: server.reportedStatus
                  ? t(`context.mcp.statusName.${server.reportedStatus}` as TranslationKey)
                  : t('context.mcp.unknown'),
              })}
            </p>
            {tools.length > 0 && (
              <table className={styles.table}>
                <thead>
                  <tr>
                    <th scope="col">{t('context.mcp.tool')}</th>
                    {columns.map((column) => (
                      <th key={column} scope="col">
                        {t(`context.mcp.column.${column}` as TranslationKey)}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {tools.map((tool) => (
                    <tr key={tool.tool}>
                      <th scope="row">
                        <code className={styles.mono}>{tool.tool}</code>
                      </th>
                      <Mark value={tool.discovered} label={t('context.mcp.column.discovered')} />
                      <Mark value={tool.enabled} label={t('context.mcp.column.enabled')} />
                      <Mark value={tool.authorized} label={t('context.mcp.column.authorized')} />
                      <Mark value={tool.exposed} label={t('context.mcp.column.exposed')} />
                      <Mark value={tool.reportedExposed} label={t('context.mcp.column.reported')} />
                      <Mark value={tool.used} label={t('context.mcp.column.used')} />
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        );
      })}
      {record.heldBack.length > 0 && (
        <p className={styles.line}>
          {t('context.mcp.heldBack', { tools: record.heldBack.join(', ') })}
        </p>
      )}
    </section>
  );
}
