import { useState, type SyntheticEvent } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import { pickFolder as nativePickFolder } from '@/lib/tauri/dialog';
import type { Workspace, WorkspaceInput } from '../types';
import styles from './Workspace.module.css';

interface Props {
  /** The workspace being edited; omit to create one. */
  initial?: Workspace;
  onSubmit: (input: WorkspaceInput) => Promise<Workspace>;
  onSaved: (workspace: Workspace) => void;
  /** Registers one more folder in a workspace just created. Absent: a workspace has one folder. */
  onAddProject?: (workspaceId: string, path: string) => Promise<Workspace>;
  onCancel: () => void;
  /** Opens the folder picker. Defaults to the operating system's native one. */
  pickFolder?: (defaultPath?: string) => Promise<string | null>;
}

/**
 * Create or edit a workspace. The project folder is chosen with the native folder picker, not
 * typed. Choosing it only records the path: nothing in the folder is read or uploaded.
 */
export function WorkspaceForm({
  initial,
  onSubmit,
  onSaved,
  onAddProject,
  onCancel,
  pickFolder = nativePickFolder,
}: Props) {
  const t = useT();
  const [name, setName] = useState(initial?.name ?? '');
  const [projectPath, setProjectPath] = useState(initial?.projectPath ?? '');
  // Folders beyond the first, only when creating: each becomes another project of the workspace.
  const [extra, setExtra] = useState<string[]>([]);
  // Set once the workspace exists, so a retry after a failed extra folder adds only what is left.
  const [created, setCreated] = useState<Workspace | null>(null);
  const [description, setDescription] = useState(initial?.description ?? '');
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  async function chooseFolder() {
    try {
      const chosen = await pickFolder(projectPath || undefined);
      if (chosen) {
        setProjectPath(chosen);
        // Suggest the folder's name for a new workspace that has no name yet.
        if (!name.trim()) setName(folderName(chosen));
      }
    } catch (e) {
      setError(errorMessage(t, e));
    }
  }

  async function addFolder() {
    try {
      const chosen = await pickFolder(extra.at(-1) ?? (projectPath || undefined));
      if (!chosen) return;
      if (!projectPath) {
        setProjectPath(chosen);
        if (!name.trim()) setName(folderName(chosen));
      } else if (chosen !== projectPath && !extra.includes(chosen)) {
        setExtra((current) => [...current, chosen]);
      }
    } catch (e) {
      setError(errorMessage(t, e));
    }
  }

  async function addExtras(workspace: Workspace, paths: string[]) {
    let latest = workspace;
    const left = [...paths];
    while (left[0] !== undefined && onAddProject) {
      latest = await onAddProject(workspace.id, left[0]);
      left.shift();
      setExtra([...left]);
    }
    return latest;
  }

  function submit(event: SyntheticEvent) {
    event.preventDefault();
    setSaving(true);
    setError(null);
    const base = created
      ? Promise.resolve(created)
      : onSubmit({ name, projectPath, description }).then((workspace) => {
          setCreated(workspace);
          return workspace;
        });
    base
      .then((workspace) => addExtras(workspace, extra))
      .then(onSaved)
      .catch((e: unknown) => {
        setError(errorMessage(t, e));
        setSaving(false);
      });
  }

  return (
    <form
      className={styles.form}
      onSubmit={submit}
      aria-label={initial ? t('workspace.form.editTitle') : t('workspace.form.createTitle')}
    >
      <h2 className={styles.panelTitle}>
        {initial ? t('workspace.form.editTitle') : t('workspace.form.createTitle')}
      </h2>
      <div className={styles.field}>
        <label htmlFor="workspace-name">{t('workspace.form.name')}</label>
        <input
          id="workspace-name"
          className={styles.control}
          value={name}
          placeholder={t('workspace.form.namePlaceholder')}
          onChange={(e) => {
            setName(e.target.value);
          }}
        />
      </div>
      {initial || !onAddProject ? (
        <div className={styles.field}>
          <label htmlFor="workspace-folder">{t('workspace.form.folder')}</label>
          <div className={styles.folderRow}>
            <input
              id="workspace-folder"
              className={styles.control}
              value={projectPath}
              readOnly
              placeholder={t('workspace.form.folderPlaceholder')}
            />
            <Button type="button" variant="secondary" onClick={() => void chooseFolder()}>
              {t('workspace.form.selectFolder')}
            </Button>
          </div>
        </div>
      ) : (
        <div className={styles.field} role="group" aria-labelledby="workspace-folders">
          <span id="workspace-folders" className={styles.fieldLabel}>
            {t('workspace.form.folders')}
          </span>
          <p className={styles.muted}>{t('workspace.form.foldersHint')}</p>
          {projectPath ? (
            <ul className={styles.folderList} aria-label={t('workspace.form.folders')}>
              {[projectPath, ...extra].map((path, index) => (
                <li key={path} className={styles.folderItem}>
                  <Icon name="folder" size={18} />
                  <span className={styles.folderText}>
                    <strong>{folderName(path)}</strong>
                    <span className={styles.folderPath} title={path}>
                      {path}
                    </span>
                  </span>
                  {index === 0 && <span className={styles.folderTag}>{t('projects.primary')}</span>}
                  <button
                    type="button"
                    className={styles.linkButton}
                    disabled={created !== null && index === 0}
                    aria-label={t('workspace.form.removeFolder', { name: folderName(path) })}
                    onClick={() => {
                      if (index === 0) {
                        const [next = '', ...rest] = extra;
                        setProjectPath(next);
                        setExtra(rest);
                      } else {
                        setExtra((current) => current.filter((p) => p !== path));
                      }
                    }}
                  >
                    <Icon name="close" size={14} />
                  </button>
                </li>
              ))}
            </ul>
          ) : (
            <p className={styles.folderEmpty}>{t('workspace.form.folderPlaceholder')}</p>
          )}
          <div>
            <Button type="button" variant="secondary" onClick={() => void addFolder()}>
              <Icon name="plus" size={14} />
              {projectPath ? t('workspace.form.addFolder') : t('workspace.form.selectFolder')}
            </Button>
          </div>
        </div>
      )}
      <div className={styles.field}>
        <label htmlFor="workspace-description">{t('workspace.form.description')}</label>
        <input
          id="workspace-description"
          className={styles.control}
          value={description}
          placeholder={t('workspace.form.descriptionPlaceholder')}
          onChange={(e) => {
            setDescription(e.target.value);
          }}
        />
      </div>
      {error && (
        <p role="alert" className={styles.notice}>
          {error}
        </p>
      )}
      <div className={styles.panelActions}>
        <Button type="button" variant="secondary" onClick={onCancel}>
          {t('common.cancel')}
        </Button>
        <Button type="submit" disabled={saving || !projectPath}>
          {initial ? t('workspace.form.save') : t('workspace.form.create')}
        </Button>
      </div>
    </form>
  );
}

/** The last component of a path, for either kind of separator. */
function folderName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? '';
}
