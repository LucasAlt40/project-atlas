import { useEffect, useMemo, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { useI18n } from '@/i18n/I18nProvider';
import type { SpecLinkDto } from '@/lib/tauri/commands';
import { groupOf, taskParts } from '../model/taskText';
import { getSpecProgress } from '../services/specService';
import type { SpecProgress } from '../types';
import styles from './StartChoice.module.css';

interface Props {
  link: SpecLinkDto;
  runId: string;
  /** Changes when the run does, so the progress is read again. */
  stamp: number;
  onOpenSpec: (specId: string) => void;
}

interface Task {
  id: string;
  text: string;
  state: 'pending' | 'reported' | 'confirmed';
}

const KEY = 'atlas.specs.runPanel.open';

function remembered(): boolean {
  try {
    return localStorage.getItem(KEY) === 'true';
  } catch {
    return false;
  }
}

function remember(open: boolean) {
  try {
    localStorage.setItem(KEY, String(open));
  } catch {
    // The panel opens closed next time: nothing to lose.
  }
}

/**
 * What the run was given of its spec and how far the tasks have got. Two things are told apart:
 * what a step *said* it finished, and what Atlas *confirms* (the run itself ended well). Closed it
 * is one line with the progress; open, the tasks come in groups, each a click away.
 */
export function SpecRunPanel({ link, runId, stamp, onOpenSpec }: Props) {
  const { t } = useI18n();
  const [progress, setProgress] = useState<SpecProgress | null>(null);
  const [open, setOpen] = useState(remembered);

  useEffect(() => {
    let current = true;
    Promise.resolve(getSpecProgress(link.specId))
      .then((all) => {
        if (current) setProgress(all.find((p) => p.workflowExecutionId === runId) ?? null);
      })
      .catch(() => {
        if (current) setProgress(null);
      });
    return () => {
      current = false;
    };
  }, [link.specId, runId, stamp]);

  const tasks: Task[] =
    progress?.tasks ?? link.tasks.map((task) => ({ ...task, state: 'pending' }));
  const groups = useMemo(() => {
    const byGroup = new Map<string, Task[]>();
    for (const task of tasks) {
      const key = groupOf(task.id);
      byGroup.set(key, [...(byGroup.get(key) ?? []), task]);
    }
    return [...byGroup.entries()];
  }, [tasks]);
  const total = tasks.length;
  const confirmed = progress?.confirmed ?? 0;
  const reported = progress?.reported ?? 0;
  const pct = (n: number) => (total === 0 ? 0 : (n / total) * 100);

  return (
    <section
      className={styles.runPanel}
      aria-label={t('specs.runPanel.title', { title: link.title })}
    >
      <header>
        <button
          type="button"
          className={styles.runToggle}
          aria-expanded={open}
          onClick={() => {
            remember(!open);
            setOpen(!open);
          }}
        >
          <Icon name={open ? 'chevronDown' : 'chevronRight'} size={14} />
          <h3>{t('specs.runPanel.title', { title: link.title })}</h3>
        </button>
        <span className={styles.runCount}>
          {t('specs.runPanel.count', { confirmed, reported, total })}
        </span>
        <Button
          variant="ghost"
          onClick={() => {
            onOpenSpec(link.specId);
          }}
        >
          {t('specs.runPanel.open')}
        </Button>
      </header>
      {total > 0 && (
        <div
          className={styles.runMeter}
          role="img"
          aria-label={t('specs.runPanel.summary', { reported, confirmed, total })}
        >
          <span data-kind="confirmed" style={{ width: `${String(pct(confirmed))}%` }} />
          <span
            data-kind="reported"
            style={{ width: `${String(Math.max(0, pct(reported) - pct(confirmed)))}%` }}
          />
        </div>
      )}
      {progress?.specChanged && (
        <p className={styles.changed} role="status">
          {t('specs.runPanel.changed')}
        </p>
      )}
      {open &&
        (total === 0 ? (
          <p className={styles.help}>{t('specs.runPanel.noTasks')}</p>
        ) : (
          <>
            <div className={styles.groups}>
              {groups.map(([group, items]) => {
                const done = items.filter((i) => i.state !== 'pending').length;
                const touched = done > 0 && done < items.length;
                return (
                  <details key={group} className={styles.group} open={touched}>
                    <summary>
                      <span className={styles.groupName}>
                        {t('specs.runPanel.group', { n: group })}
                      </span>
                      <span className={styles.groupCount}>
                        {t('specs.runPanel.groupCount', { done, total: items.length })}
                      </span>
                    </summary>
                    <ul className={styles.tasks}>
                      {items.map((task) => (
                        <TaskRow key={task.id} task={task} />
                      ))}
                    </ul>
                  </details>
                );
              })}
            </div>
            <p className={styles.help}>{t('specs.runPanel.explain')}</p>
          </>
        ))}
    </section>
  );
}

function TaskRow({ task }: { task: Task }) {
  const { t } = useI18n();
  const parts = taskParts(task.text);
  return (
    <li data-state={task.state}>
      <span className={styles.taskState} title={t(`specs.runPanel.state.${task.state}`)}>
        {t(`specs.runPanel.state.${task.state}`)}
      </span>
      <span className={styles.taskId}>{task.id}</span>
      <div className={styles.taskBody}>
        <p className={styles.taskTitle}>{parts.title}</p>
        {(parts.requirement !== null || parts.dependsOn.length > 0) && (
          <div className={styles.taskTags}>
            {parts.requirement && (
              <span title={t('specs.runPanel.requirement')}>{parts.requirement}</span>
            )}
            {parts.dependsOn.length > 0 && (
              <span>{t('specs.runPanel.after', { ids: parts.dependsOn.join(', ') })}</span>
            )}
          </div>
        )}
        {(parts.verification !== null || parts.assumption !== null) && (
          <details className={styles.taskMore}>
            <summary>{t('specs.runPanel.more')}</summary>
            {parts.verification !== null && (
              <p>
                <strong>{t('specs.runPanel.verify')}</strong> {parts.verification}
              </p>
            )}
            {parts.assumption !== null && (
              <p>
                <strong>{t('specs.runPanel.assumption')}</strong> {parts.assumption}
              </p>
            )}
          </details>
        )}
      </div>
    </li>
  );
}
