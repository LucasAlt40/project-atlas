import { useT } from '@/i18n/I18nProvider';
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
 * Where a run of a task over several projects stands (ADR 0043): one run per project, in order.
 * Each project that has a run can be opened; the ones not started yet say so.
 */
export function ChainBar({ run, runs, onShow }: Props) {
  const t = useT();
  const chain = run.chain;
  if (!chain) return null;
  const ofChain = runs.filter((r) => r.chain?.id === chain.id);
  return (
    <div className={styles.chainBar} role="group" aria-label={t('runPlan.chain.label')}>
      <span>
        {t('runPlan.chain.position', {
          position: chain.position + 1,
          count: chain.projects.length,
        })}
      </span>
      {chain.projects.map((project, index) => {
        const projectRun = ofChain.find((r) => r.chain?.position === index);
        const status = projectRun
          ? t(RUN_STATUS_LABEL[projectRun.status])
          : t('runPlan.chain.waiting');
        return (
          <button
            key={project.id}
            type="button"
            className={styles.chainStep}
            aria-current={index === chain.position}
            disabled={!projectRun}
            title={status}
            onClick={() => {
              if (projectRun) onShow(projectRun.id);
            }}
          >
            {index + 1}. {project.name} · {status}
          </button>
        );
      })}
    </div>
  );
}
