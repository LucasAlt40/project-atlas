import { useCallback, useEffect, useState } from 'react';
import { Badge, type BadgeTone } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import type { TranslationKey } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import type { McpConfigDialectDto, McpSyncPlanDto, McpSyncReportDto } from '@/lib/tauri/commands';
import { applyMcpSync, planMcpSync } from '../../services/mcpService';
import { skipReason } from './ConfigIo';
import styles from './McpHub.module.css';

const ORDER: McpConfigDialectDto[] = ['claude', 'gemini', 'open_code', 'codex', 'copilot'];

/** How many files would change and how many servers would be added in all. */
export function pendingOf(plan: McpSyncPlanDto): {
  files: number;
  additions: number;
  imports: number;
} {
  const additions = plan.servers.reduce((sum, s) => sum + s.missingFrom.length, 0);
  const files = plan.runtimes.filter((r) =>
    plan.servers.some((s) => s.missingFrom.includes(r.dialect)),
  ).length;
  // A server only a CLI has also becomes a connection of Atlas.
  const imports = plan.servers.filter((s) => !s.inAtlas && s.foundIn.length > 0).length;
  return { files, additions, imports };
}

/**
 * The same servers everywhere: what each CLI has (Claude Code, Gemini, OpenCode, Codex) next to
 * what Atlas has, and one button that fills in what is missing. Nothing is overwritten; each file
 * that changes is copied first; a CLI with no file of its own is not written to.
 */
export function SyncRuntimes({
  workspaceId,
  onChanged,
}: {
  workspaceId: string;
  onChanged: () => void;
}) {
  const { t } = useI18n();
  const [plan, setPlan] = useState<McpSyncPlanDto | null>(null);
  const [report, setReport] = useState<McpSyncReportDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    planMcpSync(workspaceId)
      .then(setPlan)
      .catch((cause: unknown) => {
        setError(errorMessage(t, cause));
      });
  }, [workspaceId, t]);
  useEffect(() => {
    load();
  }, [load]);

  function run() {
    setBusy(true);
    setError(null);
    applyMcpSync(workspaceId)
      .then((result) => {
        setReport(result);
        onChanged();
        load();
      })
      .catch((cause: unknown) => {
        setError(errorMessage(t, cause));
      })
      .finally(() => {
        setBusy(false);
      });
  }

  if (!plan) {
    return error ? (
      <p role="alert" className={styles.error}>
        {error}
      </p>
    ) : (
      <p className={styles.muted} role="status">
        {t('hub.sync.reading')}
      </p>
    );
  }

  const pending = pendingOf(plan);
  const name = (d: McpConfigDialectDto) => t(`hub.runtime.${d}` as TranslationKey);
  return (
    <div className={styles.stack}>
      <p className={styles.muted}>{t('hub.sync.note')}</p>
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
      <ul className={styles.cliStates} aria-label={t('hub.sync.cliState')}>
        {plan.runtimes.map((runtime) => (
          <li key={runtime.dialect} className={styles.cliState}>
            <strong>{name(runtime.dialect)}</strong>
            <Badge
              dot
              mono
              caps={false}
              tone={runtime.error ? 'warning' : runtime.available ? 'success' : 'neutral'}
              {...(runtime.path ? { title: runtime.path } : {})}
            >
              {runtime.error
                ? t('hub.sync.unreadable')
                : runtime.available
                  ? t('hub.sync.available')
                  : t('hub.sync.notFound')}
            </Badge>
            <code className={styles.cliPath}>{runtime.path ?? '—'}</code>
          </li>
        ))}
      </ul>

      {plan.servers.length === 0 ? (
        <p className={styles.muted}>{t('hub.sync.nothing')}</p>
      ) : (
        <div className={styles.matrixWrap}>
          <table className={styles.matrix}>
            <thead>
              <tr>
                <th scope="col">{t('hub.sync.server')}</th>
                <th scope="col">Atlas</th>
                {ORDER.map((d) => (
                  <th key={d} scope="col">
                    {name(d)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {plan.servers.map((server) => (
                <tr key={server.name}>
                  <th scope="row">
                    <code className={styles.mono}>{server.name}</code>
                  </th>
                  <Cell
                    state={server.inAtlas ? 'has' : 'imports'}
                    label={`${server.name}: Atlas`}
                  />
                  {ORDER.map((d) => {
                    const runtime = plan.runtimes.find((r) => r.dialect === d);
                    const state = server.foundIn.includes(d)
                      ? 'has'
                      : server.missingFrom.includes(d)
                        ? 'adds'
                        : runtime?.available
                          ? 'blocked'
                          : 'none';
                    return <Cell key={d} state={state} label={`${server.name}: ${name(d)}`} />;
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {plan.skipped.length > 0 && (
        <details>
          <summary className={styles.muted}>
            {t('hub.sync.skipped', { n: plan.skipped.length })}
          </summary>
          <ul className={styles.plainList}>
            {plan.skipped.map((s) => (
              <li key={`${s.dialect}-${s.name}`} className={styles.muted}>
                <strong>{s.name}</strong> ({name(s.dialect)}) — {skipReason(t, s.reason)}
              </li>
            ))}
          </ul>
        </details>
      )}

      <p className={styles.callout}>
        <Icon name="warning" size={14} /> {t('hub.sync.trust')}
      </p>

      <div className={styles.dialogFoot}>
        <Button disabled={busy || (pending.additions === 0 && pending.imports === 0)} onClick={run}>
          <Icon name="refresh" size={14} />{' '}
          {pending.additions === 0 && pending.imports === 0
            ? t('hub.sync.inSync')
            : t('hub.sync.action', {
                additions: pending.additions,
                files: pending.files,
                imports: pending.imports,
              })}
        </Button>
      </div>

      {report && (
        <div className={styles.stack} role="status">
          <p className={styles.ok}>
            {t('hub.sync.done', {
              imported: report.imported.length,
              added: report.applied.reduce((sum, a) => sum + a.added.length, 0),
            })}
          </p>
          <ul className={styles.plainList}>
            {report.applied.map((a) => (
              <li key={a.dialect} className={styles.muted}>
                <strong>{name(a.dialect)}</strong>:{' '}
                {a.error
                  ? errorMessage(t, { code: a.error, params: {} })
                  : a.added.length === 0
                    ? t('hub.sync.nothingToAdd')
                    : a.added.join(', ')}
                {a.backup && ` · ${t('hub.config.backup', { path: a.backup })}`}
              </li>
            ))}
          </ul>
          {report.imported.some((i) => i.secretsMissing.length > 0) && (
            <p className={styles.callout}>
              {t('hub.sync.secrets', {
                names: report.imported
                  .flatMap((i) => i.secretsMissing.map((s) => `${i.name}:${s}`))
                  .join(', '),
              })}
            </p>
          )}
        </div>
      )}
    </div>
  );
}

const CELL_TONE: Record<'has' | 'adds' | 'imports' | 'blocked' | 'none', BadgeTone> = {
  has: 'success',
  adds: 'accent',
  imports: 'info',
  blocked: 'neutral',
  none: 'neutral',
};

function Cell({
  state,
  label,
}: {
  state: 'has' | 'adds' | 'imports' | 'blocked' | 'none';
  label: string;
}) {
  const { t } = useI18n();
  const text = t(`hub.sync.cell.${state}` as TranslationKey);
  return (
    <td data-state={state} aria-label={`${label}: ${text}`} title={text}>
      <Badge dot mono caps={false} tone={CELL_TONE[state]}>
        {text}
      </Badge>
    </td>
  );
}
