import { useEffect, useMemo, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import type { Agent } from '@/features/agents/types';
import type { Workspace } from '@/features/workspace/types';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import { agentsOf, suggestedAuthor } from '../model/agents';
import { stageName } from '../model/messages';
import { importSpec, listImportable } from '../services/specService';
import type { ImportCandidate, SpecView } from '../types';
import styles from './Specs.module.css';

interface Props {
  workspace: Workspace;
  agents: Agent[];
  onClose: () => void;
  onImported: (view: SpecView) => void;
}

/** What a problem code of a change folder means, in words. */
function problemText(t: ReturnType<typeof useI18n>['t'], code: string): string {
  const [kind = '', ...rest] = code.split(':');
  const what = rest.join(':');
  switch (kind) {
    case 'missing':
      return t('specs.import.problem.missing', { stage: stageName(t, what) });
    case 'unrecognized':
      return t('specs.import.problem.unrecognized', { path: what });
    case 'schema':
      return t('specs.import.problem.schema', { schema: what });
    case 'too_large':
      return t('specs.import.problem.too_large', { path: what });
    default:
      return code;
  }
}

/** The OpenSpec changes already in the project. They are read, never changed or approved. */
export function ImportDialog({ workspace, agents, onClose, onImported }: Props) {
  const { t } = useI18n();
  const inWorkspace = useMemo(() => agentsOf(workspace, agents), [workspace, agents]);
  const [projectId, setProjectId] = useState(workspace.projects[0]?.id ?? '');
  const [author, setAuthor] = useState(suggestedAuthor(inWorkspace)?.id ?? '');
  const [loaded, setLoaded] = useState<{ projectId: string; list: ImportCandidate[] } | null>(null);
  const found = loaded?.projectId === projectId ? loaded.list : null;
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<unknown>(null);

  useEffect(() => {
    let current = true;
    listImportable(workspace.id, projectId)
      .then((list) => {
        if (current) setLoaded({ projectId, list });
      })
      .catch((failure: unknown) => {
        if (current) {
          setError(failure);
          setLoaded({ projectId, list: [] });
        }
      });
    return () => {
      current = false;
    };
  }, [workspace.id, projectId]);

  async function bring(change: ImportCandidate) {
    if (busy) return;
    setBusy(change.changeId);
    setError(null);
    try {
      onImported(await importSpec(workspace.id, projectId, change.changeId, author, null));
    } catch (failure: unknown) {
      setError(failure);
      setBusy(null);
    }
  }

  return (
    <Modal label={t('specs.import')} onClose={onClose} size="wide">
      <div className={styles.form}>
        <h2 className={styles.dialogTitle}>{t('specs.import')}</h2>
        <p className={styles.muted}>{t('specs.import.intro')}</p>
        <div className={styles.fieldRow}>
          {workspace.projects.length > 1 && (
            <label className={styles.field}>
              <span>{t('specs.new.project')}</span>
              <select
                value={projectId}
                onChange={(event) => {
                  setProjectId(event.target.value);
                }}
              >
                {workspace.projects.map((project) => (
                  <option key={project.id} value={project.id}>
                    {project.name}
                  </option>
                ))}
              </select>
            </label>
          )}
          <label className={styles.field}>
            <span>{t('specs.new.author')}</span>
            <select
              value={author}
              onChange={(event) => {
                setAuthor(event.target.value);
              }}
            >
              {inWorkspace.map((agent) => (
                <option key={agent.id} value={agent.id}>
                  {agent.name}
                </option>
              ))}
            </select>
          </label>
        </div>
        {found === null && <p className={styles.muted}>{t('specs.import.reading')}</p>}
        {found?.length === 0 && (
          <p className={styles.muted} role="status">
            {t('specs.import.none')}
          </p>
        )}
        <ul className={styles.importList}>
          {found?.map((change) => (
            <li key={change.changeId} className={styles.importItem}>
              <div>
                <span className={styles.mono}>{change.changeId}</span>
                <div className={styles.importDocs}>
                  {change.artifacts.map((id) => stageName(t, id)).join(' · ') ||
                    t('specs.import.noDocuments')}
                </div>
                {change.problems.length > 0 && (
                  <ul className={styles.problems}>
                    {change.problems.map((code) => (
                      <li key={code}>{problemText(t, code)}</li>
                    ))}
                  </ul>
                )}
              </div>
              {change.importedAs ? (
                <span className={styles.muted}>{t('specs.import.already')}</span>
              ) : (
                <Button
                  variant="secondary"
                  disabled={busy !== null || author === '' || change.artifacts.length === 0}
                  onClick={() => void bring(change)}
                >
                  {t('specs.import.action')}
                </Button>
              )}
            </li>
          ))}
        </ul>
        <p className={styles.note}>{t('specs.import.readOnly')}</p>
        {error !== null && (
          <p className={styles.error} role="alert">
            {errorMessage(t, error)}
          </p>
        )}
      </div>
    </Modal>
  );
}
