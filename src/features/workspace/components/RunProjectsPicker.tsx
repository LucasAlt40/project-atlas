import { useDismiss } from '@/hooks/useDismiss';
import { useId, useRef, useState } from 'react';
import { Icon } from '@/components/ui/Icon';
import { SegmentedControl } from '@/components/ui/SegmentedControl';
import { useT } from '@/i18n/I18nProvider';
import type { WorkspaceProjectDto } from '@/lib/tauri/commands';
import {
  allWorking,
  hasManyProjects,
  moved,
  oneWorking,
  usageOf,
  withUse,
  type ProjectUse,
  type RunPlan,
} from '../model/runPlan';
import styles from './Projects.module.css';

interface Props {
  projects: WorkspaceProjectDto[];
  plan: RunPlan;
  onChange: (plan: RunPlan) => void;
  disabled?: boolean;
}

/**
 * Which projects a workflow run changes (one run each, in order) and which it only consults
 * (ADR 0043). Shown only for a workspace that has more than one project.
 */
export function RunProjectsPicker({ projects, plan, onChange, disabled = false }: Props) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const bodyId = useId();
  const root = useRef<HTMLDivElement>(null);
  useDismiss(open, root, () => {
    setOpen(false);
  });
  if (projects.length < 2) return null;

  const workspace = { projects };
  const name = (id: string) => projects.find((p) => p.id === id)?.name ?? id;
  const working = plan.workingProjectIds;
  const summary =
    working.length > 1
      ? t('runPlan.summary.chain', { order: working.map(name).join(' → ') })
      : t('runPlan.summary.one', { name: name(working[0] ?? '') });
  const badge =
    working.length > 1
      ? t('runPlan.badge.chain', { count: working.length })
      : t('runPlan.badge.one', { name: name(working[0] ?? '') });
  const listed = [
    ...working.map((id) => projects.find((p) => p.id === id)).filter((p) => p !== undefined),
    ...projects.filter((p) => !working.includes(p.id)),
  ];

  return (
    <div className={styles.scope} ref={root}>
      <button
        type="button"
        className={styles.scopeToggle}
        data-multi={working.length > 1}
        aria-expanded={open}
        aria-controls={bodyId}
        aria-label={`${t('runPlan.label')}: ${summary}`}
        title={summary}
        onClick={() => {
          setOpen((o) => !o);
        }}
      >
        <Icon name="folder" size={15} />
        <span className={styles.scopeName}>{badge}</span>
        <Icon name="chevronDown" size={14} />
      </button>
      {open && (
        <fieldset id={bodyId} className={styles.scopeBody} disabled={disabled}>
          <legend className={styles.muted}>{t('runPlan.label')}</legend>
          {hasManyProjects(workspace) && (
            <p className={styles.muted}>{t('runPlan.many', { count: projects.length })}</p>
          )}
          <ol className={styles.planList}>
            {listed.map((project) => {
              const use = usageOf(plan, project.id);
              const position = working.indexOf(project.id);
              return (
                <li key={project.id} className={styles.planRow}>
                  <span className={styles.planName}>
                    {use === 'work' && working.length > 1 && (
                      <span className={styles.planOrder}>{position + 1}</span>
                    )}
                    {project.role
                      ? t('scope.projectRole', {
                          name: project.name,
                          role: t(`projects.role.${project.role}`),
                        })
                      : project.name}
                  </span>
                  <span className={styles.planControls}>
                    <SegmentedControl<ProjectUse>
                      label={t('runPlan.useOf', { name: project.name })}
                      value={use}
                      segments={[
                        {
                          value: 'work',
                          label: t('runPlan.use.work'),
                          title: t('runPlan.use.work.hint'),
                        },
                        {
                          value: 'read',
                          label: t('runPlan.use.read'),
                          title: t('runPlan.use.read.hint'),
                          disabled: use === 'work' && working.length === 1,
                        },
                        {
                          value: 'none',
                          label: t('runPlan.use.none'),
                          disabled: use === 'work' && working.length === 1,
                        },
                      ]}
                      onChange={(next) => {
                        onChange(withUse(plan, project.id, next, workspace));
                      }}
                    />
                    {use === 'work' && working.length > 1 && (
                      <>
                        <button
                          type="button"
                          className={styles.planMove}
                          aria-label={t('runPlan.earlier', { name: project.name })}
                          title={t('runPlan.earlier', { name: project.name })}
                          disabled={position === 0}
                          onClick={() => {
                            onChange(moved(plan, project.id, -1));
                          }}
                        >
                          <Icon name="chevronUp" size={14} />
                        </button>
                        <button
                          type="button"
                          className={styles.planMove}
                          aria-label={t('runPlan.later', { name: project.name })}
                          title={t('runPlan.later', { name: project.name })}
                          disabled={position === working.length - 1}
                          onClick={() => {
                            onChange(moved(plan, project.id, 1));
                          }}
                        >
                          <Icon name="chevronDown" size={14} />
                        </button>
                      </>
                    )}
                  </span>
                </li>
              );
            })}
          </ol>
          <div className={styles.toolbar}>
            <button
              type="button"
              className={styles.scopeToggle}
              onClick={() => {
                onChange(allWorking(workspace));
              }}
            >
              {t('runPlan.allWork')}
            </button>
            <button
              type="button"
              className={styles.scopeToggle}
              onClick={() => {
                onChange(oneWorking(workspace, working[0]));
              }}
            >
              {t('runPlan.oneWorks')}
            </button>
          </div>
          <p className={styles.muted}>{t('runPlan.hint')}</p>
        </fieldset>
      )}
    </div>
  );
}
