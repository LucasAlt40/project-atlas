import { useState, type SyntheticEvent } from 'react';
import { Button } from '@/components/ui/Button';
import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import { pickFolder as nativePickFolder } from '@/lib/tauri/dialog';
import type { ProjectRoleDto, WorkspaceProjectDto } from '@/lib/tauri/commands';
import styles from './Workspace.module.css';
import projects from './Projects.module.css';

export const PROJECT_ROLES: readonly ProjectRoleDto[] = [
  'frontend',
  'backend',
  'bff',
  'service',
  'library',
  'infrastructure',
  'other',
];

export interface ProjectFormValues {
  name: string;
  path: string;
  role: ProjectRoleDto | null;
  dependsOn: string[];
}

interface Props {
  /** The project being edited; omit to add one. */
  initial?: WorkspaceProjectDto;
  /** The other projects of the workspace, to declare what this one uses. */
  others: WorkspaceProjectDto[];
  onSubmit: (values: ProjectFormValues) => Promise<void>;
  onCancel: () => void;
  pickFolder?: (defaultPath?: string) => Promise<string | null>;
}

/**
 * Add or edit a project of the workspace. The folder is chosen with the native picker, never
 * typed. Registering a folder only records its path: nothing in it is read, uploaded or changed.
 */
export function ProjectForm({
  initial,
  others,
  onSubmit,
  onCancel,
  pickFolder = nativePickFolder,
}: Props) {
  const t = useT();
  const [name, setName] = useState(initial?.name ?? '');
  const [path, setPath] = useState(initial?.path ?? '');
  const [role, setRole] = useState<ProjectRoleDto | ''>(initial?.role ?? '');
  const [dependsOn, setDependsOn] = useState<string[]>(initial?.dependsOn ?? []);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  async function chooseFolder() {
    try {
      const chosen = await pickFolder(path || undefined);
      if (chosen) setPath(chosen);
    } catch (e) {
      setError(errorMessage(t, e));
    }
  }

  function submit(event: SyntheticEvent) {
    event.preventDefault();
    if (!path) return;
    setSaving(true);
    setError(null);
    onSubmit({ name: name.trim(), path, role: role || null, dependsOn })
      .catch((e: unknown) => {
        setError(errorMessage(t, e));
      })
      .finally(() => {
        setSaving(false);
      });
  }

  const fieldId = (suffix: string) => `project-${initial?.id ?? 'new'}-${suffix}`;

  return (
    <form
      className={styles.form}
      onSubmit={submit}
      aria-label={initial ? t('projects.edit', { name: initial.name }) : t('projects.add')}
    >
      <div className={styles.field}>
        <label htmlFor={fieldId('folder')}>{t('projects.form.folder')}</label>
        <div className={styles.folderRow}>
          <input
            id={fieldId('folder')}
            className={styles.control}
            value={path}
            readOnly
            placeholder={t('workspace.form.folderPlaceholder')}
          />
          <Button type="button" variant="secondary" onClick={() => void chooseFolder()}>
            {t('projects.form.selectFolder')}
          </Button>
        </div>
      </div>
      <div className={styles.field}>
        <label htmlFor={fieldId('name')}>{t('projects.form.name')}</label>
        <input
          id={fieldId('name')}
          className={styles.control}
          value={name}
          placeholder={t('projects.form.namePlaceholder')}
          onChange={(e) => {
            setName(e.target.value);
          }}
        />
      </div>
      <div className={styles.field}>
        <label htmlFor={fieldId('role')}>{t('projects.form.role')}</label>
        <select
          id={fieldId('role')}
          className={styles.control}
          value={role}
          onChange={(e) => {
            setRole(e.target.value as ProjectRoleDto | '');
          }}
        >
          <option value="">{t('projects.form.noRole')}</option>
          {PROJECT_ROLES.map((r) => (
            <option key={r} value={r}>
              {t(`projects.role.${r}`)}
            </option>
          ))}
        </select>
      </div>
      {others.length > 0 && (
        <div className={styles.field} role="group" aria-label={t('projects.form.dependsOn')}>
          <span>{t('projects.form.dependsOn')}</span>
          <div className={projects.checks}>
            {others.map((other) => (
              <label key={other.id} className={projects.check}>
                <input
                  type="checkbox"
                  checked={dependsOn.includes(other.id)}
                  onChange={(e) => {
                    setDependsOn((current) =>
                      e.target.checked
                        ? [...current, other.id]
                        : current.filter((id) => id !== other.id),
                    );
                  }}
                />
                {other.name}
              </label>
            ))}
          </div>
          <p className={projects.muted}>{t('projects.form.dependsOnHint')}</p>
        </div>
      )}
      {error && (
        <p role="alert" className={styles.notice}>
          {error}
        </p>
      )}
      <div className={styles.panelActions}>
        <Button type="button" variant="secondary" onClick={onCancel}>
          {t('common.cancel')}
        </Button>
        <Button type="submit" disabled={saving || !path}>
          {initial ? t('projects.form.save') : t('projects.form.add')}
        </Button>
      </div>
    </form>
  );
}
