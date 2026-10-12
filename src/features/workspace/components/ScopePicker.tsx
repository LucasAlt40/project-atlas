import { useId, useState } from 'react';
import { Icon } from '@/components/ui/Icon';
import { useT } from '@/i18n/I18nProvider';
import type { WorkspaceProjectDto } from '@/lib/tauri/commands';
import { consultsAll, withRead, withWorking, type ScopeChoice } from '../model/scope';
import styles from './Projects.module.css';

interface Props {
  projects: WorkspaceProjectDto[];
  choice: ScopeChoice;
  onChange: (choice: ScopeChoice) => void;
  disabled?: boolean;
}

/**
 * Where an agent works and what else it may look at. The project it works in is the only one it
 * changes (in its isolated worktree); the others are consulted. Shown only for a workspace that
 * has more than one project: with one there is nothing to choose.
 */
export function ScopePicker({ projects, choice, onChange, disabled = false }: Props) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const bodyId = useId();
  if (projects.length < 2) return null;

  const name = (id: string) => projects.find((p) => p.id === id)?.name ?? id;
  const others = projects.filter((p) => p.id !== choice.workingProjectId);
  const multi = choice.readProjectIds.length > 0;
  const working = name(choice.workingProjectId);
  const summary = multi
    ? t('scope.with', { working, others: choice.readProjectIds.map(name).join(', ') })
    : t('scope.only', { working });

  return (
    <div className={styles.scope}>
      <button
        type="button"
        className={styles.scopeToggle}
        data-multi={multi}
        aria-expanded={open}
        aria-controls={bodyId}
        aria-label={`${t('scope.label')}: ${summary}`}
        onClick={() => {
          setOpen((o) => !o);
        }}
      >
        <Icon name="stack" size={14} />
        {multi ? t('scope.badge', { working, count: choice.readProjectIds.length }) : working}
      </button>
      {open && (
        <fieldset id={bodyId} className={styles.scopeBody} disabled={disabled}>
          <legend className={styles.muted}>{t('scope.label')}</legend>
          <label className={styles.check}>
            {t('scope.working')}
            <select
              aria-label={t('scope.working')}
              value={choice.workingProjectId}
              onChange={(e) => {
                onChange(withWorking(choice, e.target.value));
              }}
            >
              {projects.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.role
                    ? t('scope.projectRole', { name: p.name, role: t(`projects.role.${p.role}`) })
                    : p.name}
                </option>
              ))}
            </select>
          </label>
          <div role="group" aria-label={t('scope.reads')}>
            <span className={styles.muted}>{t('scope.reads')}</span>
            <div className={styles.checks}>
              {others.map((p) => (
                <label key={p.id} className={styles.check}>
                  <input
                    type="checkbox"
                    checked={choice.readProjectIds.includes(p.id)}
                    onChange={(e) => {
                      onChange(withRead(choice, p.id, e.target.checked));
                    }}
                  />
                  {p.name}
                </label>
              ))}
            </div>
            <div className={styles.toolbar}>
              <button
                type="button"
                className={styles.scopeToggle}
                onClick={() => {
                  onChange(consultsAll(choice, { projects }));
                }}
              >
                {t('scope.readAll')}
              </button>
              <button
                type="button"
                className={styles.scopeToggle}
                onClick={() => {
                  onChange({ ...choice, readProjectIds: [] });
                }}
              >
                {t('scope.readNone')}
              </button>
            </div>
          </div>
          <p className={styles.muted}>{t('scope.hint')}</p>
        </fieldset>
      )}
    </div>
  );
}
