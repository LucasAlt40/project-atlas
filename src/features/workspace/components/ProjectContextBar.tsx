import { useEffect, useState } from 'react';
import { Icon } from '@/components/ui/Icon';
import { Button } from '@/components/ui/Button';
import { HarnessStats } from '@/features/harness/components/HarnessStats';
import { HealthBadge } from '@/features/harness/components/HealthBadge';
import { StaleNotice } from '@/features/harness/components/StaleNotice';
import { InitializeProjectModal } from '@/features/harness/components/InitializeProjectModal';
import { RefreshHarnessModal } from '@/features/harness/components/RefreshHarnessModal';
import harnessStyles from '@/features/harness/components/Harness.module.css';
import { useProjectHarness } from '@/features/harness/hooks/useProjectHarness';
import { useT } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import { getProjectContext, refreshProjectHarness } from '../services/workspaceService';
import type { ProjectContext, Workspace } from '../types';
import styles from './Workspace.module.css';

interface Loaded {
  error: boolean;
  context?: ProjectContext;
}

/**
 * The workspace's project: folder name, path and the technologies recognised from marker files
 * in the folder. Local and deterministic: no model is asked and nothing is uploaded.
 */
export function ProjectContextBar({ workspace }: { workspace: Workspace }) {
  const t = useT();
  const { id, projectPath } = workspace;
  // What was loaded, and for which workspace folder; anything else is "still loading".
  const [loaded, setLoaded] = useState<{ key: string; result: Loaded } | null>(null);
  const key = `${id}\u0000${projectPath}`;
  const harness = useProjectHarness(id, projectPath);
  const [initializing, setInitializing] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [fixing, setFixing] = useState(false);
  const [fixError, setFixError] = useState<string | null>(null);

  /**
   * Brings the Harness up to date in one click. The refresh keeps the user's own files and
   * corrections; when it cannot apply by itself (there is something to decide) the review opens.
   */
  function fixStale() {
    setFixing(true);
    setFixError(null);
    refreshProjectHarness(id, true)
      .then((outcome) => {
        if (outcome.applied) harness.replace(outcome.applied.summary);
        else setRefreshing(true);
      })
      .catch((e: unknown) => {
        setFixError(errorMessage(t, e));
      })
      .finally(() => {
        setFixing(false);
      });
  }

  useEffect(() => {
    let cancelled = false;
    getProjectContext(id)
      .then((context) => {
        if (!cancelled) setLoaded({ key, result: { error: false, context } });
      })
      .catch(() => {
        if (!cancelled) setLoaded({ key, result: { error: true } });
      });
    return () => {
      cancelled = true;
    };
  }, [id, key]);

  const result = loaded?.key === key ? loaded.result : undefined;
  const context = result?.context;

  return (
    <section className={styles.context} aria-label={t('project.context')}>
      <strong className={styles.contextName}>{context?.name ?? workspace.name}</strong>
      <span className={styles.pathChip} title={`${t('project.path')}: ${workspace.projectPath}`}>
        <Icon name="folder" size={14} />
        {workspace.projectPath}
      </span>
      <span className={styles.divider} aria-hidden="true" />
      {context && (
        <span className={styles.technologies}>
          {context.technologies.length > 0 ? (
            <>
              <span className={styles.muted}>{t('project.detected')}:</span>
              <ul aria-label={t('project.detected')}>
                {context.technologies.map((tech) => (
                  <li key={tech}>{tech}</li>
                ))}
              </ul>
            </>
          ) : (
            <span className={styles.muted}>{t('project.nothingDetected')}</span>
          )}
        </span>
      )}
      {result?.error && <span className={styles.notice}>{t('project.folderMissing')}</span>}
      <div className={harnessStyles.status} role="group" aria-label={t('harness.title')}>
        <span className={harnessStyles.statusLabel}>{t('harness.title')}</span>
        {harness.state.status === 'ready' && (
          <>
            <strong
              className={harnessStyles.statusValue}
              data-status={harness.state.summary.status}
            >
              {harness.state.summary.status === 'initialized' ? '✓' : '⚠'}{' '}
              {t(`harness.status.${harness.state.summary.status}`)}
            </strong>
            {harness.state.summary.health && <HealthBadge health={harness.state.summary.health} />}
            {harness.state.summary.status === 'initialized' &&
              harness.state.summary.stack.length > 0 && (
                <span className={styles.muted}>{harness.state.summary.stack.join(' · ')}</span>
              )}
            {harness.state.summary.status === 'initialized' ? (
              <>
                <Button
                  onClick={() => {
                    setInitializing(true);
                  }}
                >
                  {t('harness.update')}
                </Button>
                <Button
                  onClick={() => {
                    setRefreshing(true);
                  }}
                >
                  {t('harness.refresh')}
                </Button>
              </>
            ) : (
              <Button
                onClick={() => {
                  setInitializing(true);
                }}
              >
                {harness.state.summary.status === 'needs_review'
                  ? t('harness.openReview')
                  : t('harness.initialize')}
              </Button>
            )}
          </>
        )}
        {harness.state.status === 'error' && (
          <span className={styles.notice}>{t('harness.loadFailed')}</span>
        )}
      </div>
      {harness.state.status === 'ready' && harness.state.summary.status === 'initialized' && (
        <div className={styles.contextFoot}>
          {harness.state.summary.stats && (
            <HarnessStats
              stats={harness.state.summary.stats}
              analyzedAt={harness.state.summary.analyzedAt}
            />
          )}
          {harness.state.summary.staleness && (
            <StaleNotice
              staleness={harness.state.summary.staleness}
              fix={{
                busy: fixing,
                error: fixError,
                onFix: fixStale,
                onReview: () => {
                  setRefreshing(true);
                },
              }}
            />
          )}
        </div>
      )}
      {refreshing && (
        <RefreshHarnessModal
          workspaceId={id}
          onClose={() => {
            setRefreshing(false);
          }}
          onApplied={harness.replace}
        />
      )}
      {initializing && (
        <InitializeProjectModal
          workspaceId={id}
          onClose={() => {
            setInitializing(false);
          }}
          onInitialized={harness.replace}
        />
      )}
    </section>
  );
}
