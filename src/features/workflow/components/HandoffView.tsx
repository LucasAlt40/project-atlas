import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import { shortId } from '@/features/workspace/model/inspection';
import { divergence } from '../model/review';
import { nodeLabel } from '../model/status';
import type { FileChange, Handoff, WorkflowRun } from '../types';
import type { ResultFindingDto } from '@/lib/tauri/commands';
import styles from './Workflow.module.css';

const MARK: Record<FileChange['status'], string> = {
  added: 'A',
  modified: 'M',
  deleted: 'D',
  renamed: 'R',
};

export function FileLine({ file }: { file: FileChange }) {
  const { t } = useI18n();
  return (
    <span className={styles.fileLine}>
      <span
        className={styles.fileMark}
        data-status={file.status}
        title={t(`integration.file.${file.status}` as TranslationKey)}
      >
        {MARK[file.status]}
      </span>{' '}
      <span className={styles.filePath}>
        {file.oldPath ? `${file.oldPath} → ` : ''}
        {file.path}
      </span>{' '}
      <span className={styles.muted}>
        {file.binary
          ? t('integration.binary')
          : `+${String(file.additions ?? 0)} −${String(file.deletions ?? 0)}`}
      </span>
    </span>
  );
}

/** One finding as one line: its severity and category, its title, where it is and what it says. */
export function FindingText({ finding: f }: { finding: ResultFindingDto }) {
  return (
    <>
      [{f.severity}
      {f.category ? ` / ${f.category}` : ''}] {f.title ? <strong>{f.title}: </strong> : null}
      {f.description}
      {f.file && (
        <span className={styles.filePath}>
          {' '}
          ({f.file}
          {f.line ? `:${String(f.line)}` : ''})
        </span>
      )}
      {f.evidence && <span className={styles.muted}> — {f.evidence}</span>}
    </>
  );
}

/**
 * What one step handed to the next: its summary, decisions, artifacts, the files Git saw it
 * change and what it validated. Everything here was put together by the orchestrator; what the
 * agent merely claimed is shown apart, as a claim.
 */
export function HandoffView({
  run,
  handoff,
  onOpenExecution,
}: {
  run: WorkflowRun;
  handoff: Handoff;
  onOpenExecution: (executionId: string) => void;
}) {
  const { t } = useI18n();
  const { claimedOnly: claimed, detectedOnly } = divergence(handoff);
  return (
    <article
      className={styles.handoff}
      aria-label={t('handoff.title', {
        from: nodeLabel(run, handoff.fromNodeId),
        to: nodeLabel(run, handoff.toNodeId),
      })}
    >
      <h4 className={styles.handoffTitle}>
        {nodeLabel(run, handoff.fromNodeId)} → {nodeLabel(run, handoff.toNodeId)}{' '}
        <span className={styles.muted}>{t('handoff.pass', { n: handoff.iteration })}</span>
      </h4>
      <p className={styles.handoffStatus} data-status={handoff.status}>
        {handoff.kind === 'failure' ? t('handoff.failedStep') : t('handoff.result')}:{' '}
        <strong>{t(`handoff.status.${handoff.status}` as TranslationKey)}</strong>
      </p>
      <p>
        {t('handoff.executionStatus')}:{' '}
        <strong>
          {handoff.kind === 'failure'
            ? t('handoff.executionStatus.failed')
            : t('handoff.executionStatus.completed')}
        </strong>
        {handoff.outcome && (
          <>
            {' · '}
            {t('handoff.outcome')}:{' '}
            <strong data-outcome={handoff.outcome}>{handoff.outcome.toUpperCase()}</strong>
          </>
        )}
      </p>

      <h5>{t('handoff.summary')}</h5>
      <p className={styles.handoffText}>
        {handoff.summary === '' ? (handoff.failure ?? t('handoff.nothing')) : handoff.summary}
      </p>
      {handoff.failure && handoff.summary && (
        <p className={styles.handoffText}>{handoff.failure}</p>
      )}

      {handoff.decisions.length > 0 && (
        <>
          <h5>{t('handoff.decisions')}</h5>
          <ul>
            {handoff.decisions.map((d) => (
              <li key={`${d.title}-${d.decision}`} title={d.rationale}>
                {d.title ? `${d.title}: ` : ''}
                {d.decision}
              </li>
            ))}
          </ul>
        </>
      )}

      {handoff.artifacts.length > 0 && (
        <>
          <h5>{t('handoff.artifacts')}</h5>
          <ul>
            {handoff.artifacts.map((a) => (
              <li key={a.id} title={a.summary}>
                {a.name}
                {a.path && <span className={styles.muted}> · {a.path}</span>}
              </li>
            ))}
          </ul>
        </>
      )}

      <h5>{t('handoff.changedFiles')}</h5>
      {handoff.changedFiles.length === 0 && handoff.uncommittedFiles.length === 0 ? (
        <p className={styles.muted}>{t('handoff.noFiles')}</p>
      ) : (
        <ul className={styles.fileList}>
          {handoff.changedFiles.map((file) => (
            <li key={file.path}>
              <FileLine file={file} />
            </li>
          ))}
          {handoff.uncommittedFiles.map((path) => (
            <li key={`u-${path}`}>
              <span className={styles.filePath}>{path}</span>{' '}
              <span className={styles.muted}>{t('handoff.uncommitted')}</span>
            </li>
          ))}
        </ul>
      )}
      {claimed.length > 0 && (
        <p className={styles.muted}>{t('handoff.claimed', { files: claimed.join(', ') })}</p>
      )}
      {detectedOnly.length > 0 && handoff.reportedFiles.length > 0 && (
        <p className={styles.muted}>
          {t('handoff.detectedOnly', { files: detectedOnly.join(', ') })}
        </p>
      )}

      {handoff.validation && (
        <>
          <h5>{t('handoff.validation')}</h5>
          <p>
            <strong>{t(`handoff.status.${handoff.validation.status}` as TranslationKey)}</strong>
            {handoff.validation.summary ? ` — ${handoff.validation.summary}` : ''}
          </p>
          <ul>
            {handoff.validation.findings.map((f) => (
              <li key={`${f.category}-${f.title}-${f.description}`}>
                <FindingText finding={f} />
              </li>
            ))}
          </ul>
        </>
      )}

      {handoff.instructions && (
        <>
          <h5>{t('handoff.suggested')}</h5>
          <p className={styles.handoffText}>{handoff.instructions}</p>
          <p className={styles.muted}>{t('handoff.suggestedNote')}</p>
        </>
      )}

      <h5>{t('handoff.execution')}</h5>
      <p>
        <button
          type="button"
          className={styles.link}
          onClick={() => {
            onOpenExecution(handoff.fromExecutionId);
          }}
        >
          {shortId(handoff.fromExecutionId)}
        </button>
      </p>
    </article>
  );
}

export function HandoffList({
  run,
  handoffs,
  onOpenExecution,
}: {
  run: WorkflowRun;
  handoffs: Handoff[];
  onOpenExecution: (executionId: string) => void;
}) {
  const { t } = useI18n();
  if (handoffs.length === 0) return <p className={styles.muted}>{t('handoff.none')}</p>;
  return (
    <div className={styles.handoffList}>
      {handoffs.map((handoff) => (
        <HandoffView
          key={handoff.id}
          run={run}
          handoff={handoff}
          onOpenExecution={onOpenExecution}
        />
      ))}
    </div>
  );
}
