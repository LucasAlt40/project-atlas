import { Button } from '@/components/ui/Button';
import { Callout } from '@/components/ui/Callout';
import { useT } from '@/i18n/I18nProvider';
import { appErrorMessage } from '@/i18n/messages';
import type {
  ChainNoteDto,
  ChainSkipDto,
  ChainStopDto,
  ErrorCodeDto,
  IntegrationStatusDto,
} from '@/lib/tauri/commands';
import { RUN_STATUS_LABEL } from '../model/status';
import type { WorkflowRun } from '../types';
import styles from './Workflow.module.css';

interface Props {
  run: WorkflowRun;
  /** The runs of the workflow that are known, to find the chain's others among. */
  runs: WorkflowRun[];
  onShow: (runId: string) => void;
  /** Goes on with the chain from a completed run (the person's "continue"). */
  onContinue?: (runId: string) => void;
}

/**
 * Where a run of a task over several projects stands (ADR 0043): one run per project the person
 * marked, in order. Each project that has a run can be opened; a project the run before said needs
 * nothing is shown as skipped, and a project whose run could not be started ends the chain with
 * the reason. Below, what the earlier runs handed this one: the runs talk to each
 * other, and the person can read what they said.
 */
/** Where a run's code stands lets a chain that waits for it go on (mirrors `IntegrationStatus::delivery`). */
const DELIVERED: ReadonlySet<IntegrationStatusDto> = new Set<IntegrationStatusDto>([
  'not_applicable',
  'no_changes',
  'integrated',
  'kept_isolated',
]);

export function ChainBar({ run, runs, onShow, onContinue }: Props) {
  const t = useT();
  const chain = run.chain;
  if (!chain) return null;
  const ofChain = runs.filter((r) => r.chain?.id === chain.id);
  // The latest run of the chain knows every project skipped so far.
  const skipped = new Map<string, ChainSkipDto>();
  for (const r of ofChain)
    for (const skip of r.chain?.skipped ?? []) skipped.set(skip.projectId, skip);
  for (const skip of chain.skipped ?? []) skipped.set(skip.projectId, skip);
  const stopped: ChainStopDto | undefined =
    chain.stopped ?? ofChain.find((r) => r.chain?.stopped)?.chain?.stopped;
  const name = (id: string) => chain.projects.find((p) => p.id === id)?.name ?? id;
  const current = chain.projects[chain.position];
  // The chain may go on from this run: it completed, a project is left after it, and the chain
  // did not already go on (a later run, or every later project passed over by this run).
  const nextProject = chain.projects
    .slice(chain.position + 1)
    .find((p) => !skipped.has(p.id) || stopped?.projectId === p.id);
  const wentOn =
    ofChain.some((r) => (r.chain?.position ?? -1) > chain.position) ||
    (chain.skipped ?? []).some((s) => s.byExecutionId === run.id);
  // Code that was thrown away ends a chain that waited for it: trying again would not change that.
  const retryable = stopped?.code !== 'chain_changes_discarded';
  const canGoOn = run.status === 'completed' && nextProject !== undefined && !wentOn && retryable;
  const awaitingDelivery =
    canGoOn && chain.waitForDelivery === true && !DELIVERED.has(run.integration.status);
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
              : stopped?.projectId === project.id
                ? t('runPlan.chain.stopped')
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
      {stopped && (
        <Callout
          tone="danger"
          role="alert"
          title={t('runPlan.chain.stoppedTitle', { name: name(stopped.projectId) })}
        >
          {appErrorMessage(t, {
            code: stopped.code as ErrorCodeDto,
            params: stopped.params,
            detail: null,
          })}
        </Callout>
      )}
      {canGoOn && awaitingDelivery && !stopped && (
        <p className={styles.chainWaiting} role="status">
          {t('runPlan.chain.awaitingDelivery', { name: nextProject.name })}
        </p>
      )}
      {canGoOn && !awaitingDelivery && onContinue && (
        <div>
          <Button
            variant={stopped ? 'secondary' : 'primary'}
            size="sm"
            onClick={() => {
              onContinue(run.id);
            }}
          >
            {stopped
              ? t('runPlan.chain.retry', { name: nextProject.name })
              : t('runPlan.chain.continue', { name: nextProject.name })}
          </Button>
        </div>
      )}
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
