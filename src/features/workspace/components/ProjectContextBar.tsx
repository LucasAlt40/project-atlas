import { useEffect, useState } from 'react';
import { useT } from '@/i18n/I18nProvider';
import { getProjectContext } from '../services/workspaceService';
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
      <span aria-hidden="true">📁</span>
      <strong>{context?.name ?? workspace.name}</strong>
      <span className={styles.muted} title={workspace.projectPath}>
        {t('project.path')}: {workspace.projectPath}
      </span>
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
    </section>
  );
}
