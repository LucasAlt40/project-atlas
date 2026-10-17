import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { useI18n } from '@/i18n/I18nProvider';
import { failureText } from '../model/messages';
import type { FindingStatus, SpecFinding, SpecQuestion, SpecView } from '../types';
import { Findings, FindingsBrowser } from './Findings';
import { QuestionCard } from './QuestionCard';
import { PanelSection } from './PanelSection';
import styles from './Specs.module.css';

interface Props {
  view: SpecView;
  busy: boolean;
  reviewerName: string;
  onRun: () => void;
  onWaive: (note: string) => void;
  onAnswerQuestion: (question: SpecQuestion, choice: string | null, answer: string | null) => void;
  onAnswerFindings: (answers: { findingId: string; answer: string }[]) => Promise<boolean>;
  onResolve: (findingId: string, status: FindingStatus, note: string | null) => void;
  onShowFinding: (finding: SpecFinding) => void;
  onCancel: () => void;
}

/** The reviewing agent's pass over the approved documents, and what the person decides on it. */
export function ReviewPanel({
  view,
  busy,
  reviewerName,
  onRun,
  onWaive,
  onAnswerQuestion,
  onAnswerFindings,
  onResolve,
  onShowFinding,
  onCancel,
}: Props) {
  const { t } = useI18n();
  const [waiving, setWaiving] = useState(false);
  const [note, setNote] = useState('');
  const review = view.spec.aiReview;
  const run = review?.run;
  const allApproved = view.spec.artifacts.every((a) => view.phases[a.id] === 'approved');
  const pending = view.spec.questions.filter(
    (q) => q.status === 'pending' && q.source === 'review',
  );
  const open = view.spec.findings.filter((f) => f.status === 'open');
  const settled = view.spec.findings.filter((f) => f.status !== 'open');
  const canRun =
    allApproved &&
    !view.busy &&
    view.reviewerAvailable &&
    !view.spec.archived &&
    pending.length === 0;
  const whyNot = !allApproved
    ? t('specs.review.needsApproved')
    : view.busy
      ? t('specs.why.run_in_progress')
      : !view.reviewerAvailable
        ? t('specs.why.reviewer_missing')
        : null;

  return (
    <PanelSection
      title={t('specs.review.title')}
      tone={view.reviewCurrent ? 'success' : 'neutral'}
      defaultOpen={run?.state === 'running' || view.spec.findings.some((f) => f.status === 'open')}
      tag={
        run?.state === 'running'
          ? t('specs.review.running')
          : view.reviewCurrent
            ? run?.state === 'waived'
              ? t('specs.review.waived')
              : t('specs.review.done')
            : run?.state === 'completed'
              ? t('specs.review.outdated')
              : t('specs.review.notRun')
      }
    >
      <p className={styles.muted}>{t('specs.review.intro', { agent: reviewerName })}</p>
      {run?.state === 'completed' && view.reviewCurrent && run.summary && (
        <p className={styles.summary}>{run.summary}</p>
      )}
      {run?.state === 'waived' && (
        <p className={styles.warnLine}>{t('specs.review.waivedNote', { note: run.note })}</p>
      )}
      {run?.state === 'failed' && !(run.message === 'answered') && (
        <p className={styles.errorLine} role="alert">
          {failureText(t, run.message).text}
        </p>
      )}
      {pending.map((question) => (
        <QuestionCard
          key={question.id}
          question={question}
          busy={busy}
          onAnswer={(choice, answer) => {
            onAnswerQuestion(question, choice, answer);
          }}
        />
      ))}
      <div className={styles.row}>
        {run?.state === 'running' ? (
          <Button variant="secondary" disabled={busy} onClick={onCancel}>
            {t('specs.run.cancel')}
          </Button>
        ) : (
          <Button disabled={!canRun || busy} onClick={onRun}>
            {review ? t('specs.review.rerun') : t('specs.review.run')}
          </Button>
        )}
        {!waiving && run?.state !== 'running' && !view.reviewCurrent && (
          <Button
            variant="ghost"
            disabled={!allApproved || busy || !!view.spec.archived}
            onClick={() => {
              setWaiving(true);
            }}
          >
            {t('specs.review.waive')}
          </Button>
        )}
      </div>
      {whyNot && !canRun && <p className={styles.muted}>{whyNot}</p>}
      {canRun && view.reviewCurrent && run?.state === 'completed' && (
        <p className={styles.muted}>{t('specs.review.nothingChanged')}</p>
      )}
      {waiving && (
        <div className={styles.answerBox}>
          <label className={styles.field}>
            <span>{t('specs.review.waiveWhy')}</span>
            <textarea
              rows={2}
              value={note}
              maxLength={2000}
              onChange={(event) => {
                setNote(event.target.value);
              }}
            />
          </label>
          <div className={styles.row}>
            <Button
              disabled={busy || note.trim() === ''}
              onClick={() => {
                onWaive(note.trim());
                setWaiving(false);
              }}
            >
              {t('specs.review.waiveConfirm')}
            </Button>
            <Button
              variant="secondary"
              onClick={() => {
                setWaiving(false);
              }}
            >
              {t('common.cancel')}
            </Button>
          </div>
        </div>
      )}
      {run?.state === 'completed' && view.reviewCurrent && view.spec.findings.length === 0 && (
        <p className={styles.okLine}>{t('specs.review.nothingFound')}</p>
      )}
      <FindingsBrowser
        specId={view.spec.id}
        findings={open}
        busy={busy}
        onSendAnswers={onAnswerFindings}
        onResolve={onResolve}
        onShow={onShowFinding}
      />
      {settled.length > 0 && (
        <details className={styles.settled}>
          <summary>{t('specs.review.settled', { count: settled.length })}</summary>
          <Findings findings={settled} busy={busy} onResolve={onResolve} onShow={onShowFinding} />
        </details>
      )}
    </PanelSection>
  );
}
