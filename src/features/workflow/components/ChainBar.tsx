import { useT } from '@/i18n/I18nProvider';
import type { ChainNoteDto, ChainSkipDto } from '@/lib/tauri/commands';
import { RUN_STATUS_LABEL } from '../model/status';
import type { WorkflowRun } from '../types';
import styles from './Workflow.module.css';

interface Props {
  run: WorkflowRun;
  /** The runs of the workflow that are known, to find the chain's others among. */
  runs: WorkflowRun[];
  onShow: (runId: string) => void;
}

/**
 * Where a run of a task over several projects stands (ADR 0043): one run per project the person
 * marked, in order. Each project that has a run can be opened; a project the run before said needs
 * nothing is shown as skipped. Below, what the earlier runs handed this one: the runs talk to each
 * other, and the person can read what they said.
 */
export function ChainBar({ run, runs, onShow }: Props) {
  const t = useT();
  const chain = run.chain;
  if (!chain) return null;
  const ofChain = runs.filter((r) => r.chain?.id === chain.id);
  // The latest run of the chain knows every project skipped so far.
  const skipped = new Map<string, ChainSkipDto>();
  for (const r of ofChain)
    for (const skip of r.chain?.skipped ?? []) skipped.set(skip.projectId, skip);
  for (const skip of chain.skipped ?? []) skipped.set(skip.projectId, skip);
  const name = (id: string) => chain.projects.find((p) => p.id === id)?.name ?? id;
  const current = chain.projects[chain.position];
  const received: { from: string; note: ChainNoteDto }[] = chain.earlier.flatMap((step) =>
    (step.notes ?? [])
      .filter((note) => note.projectId === current?.id)
      .map((note) => ({ from: name(step.projectId), note })),
  );

  return (
    <section className={styles.chain} aria-label={t('runPlan.chain.label')}>
      <div className={styles.chainBar} role="group" aria-label={t('runPlan.chain.label')}>
        <span>
          {t('runPlan.chain.position', {
            position: chain.position + 1,
            count: chain.projects.length,
          })}
        </span>
        {chain.projects.map((project, index) => {
          const projectRun = ofChain.find((r) => r.chain?.position === index);
          const skip = skipped.get(project.id);
          const status = projectRun
            ? t(RUN_STATUS_LABEL[projectRun.status])
            : skip
              ? t('runPlan.chain.skipped')
              : t('runPlan.chain.waiting');
          const title = skip?.reason ? `${status}: ${skip.reason}` : status;
          return (
            <button
              key={project.id}
              type="button"
              className={styles.chainStep}
              aria-current={index === chain.position}
              data-skipped={skip && !projectRun ? true : undefined}
              disabled={!projectRun}
              title={title}
              onClick={() => {
                if (projectRun) onShow(projectRun.id);
              }}
            >
              {index + 1}. {project.name} · {status}
            </button>
          );
        })}
      </div>
      {chain.earlier.length > 0 && (
        <details className={styles.chainHandoff}>
          <summary>
            {received.length > 0
              ? t('runPlan.chain.received', { name: current?.name ?? '' })
              : t('runPlan.chain.receivedNone', { name: current?.name ?? '' })}
          </summary>
          {received.map(({ from, note }) => (
            <div key={from} className={styles.chainNote}>
              <strong>{t('runPlan.chain.from', { name: from })}</strong>
              {note.done && <p>{note.done}</p>}
              {note.todo.length > 0 && (
                <ul>
                  {note.todo.map((item) => (
                    <li key={item}>{item}</li>
                  ))}
                </ul>
              )}
            </div>
          ))}
        </details>
      )}
    </section>
  );
}
