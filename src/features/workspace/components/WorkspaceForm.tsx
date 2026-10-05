import { useState, type SyntheticEvent } from 'react';
import { Button } from '@/components/ui/Button';
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
  onCancel,
  pickFolder = nativePickFolder,
}: Props) {
  const t = useT();
  const [name, setName] = useState(initial?.name ?? '');
  const [projectPath, setProjectPath] = useState(initial?.projectPath ?? '');
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

  function submit(event: SyntheticEvent) {
    event.preventDefault();
    setSaving(true);
    setError(null);
    onSubmit({ name, projectPath, description })
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
        <Button type="submit" disabled={saving}>
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
