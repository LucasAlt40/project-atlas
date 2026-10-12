import { useEffect, useState } from 'react';
import { useI18n } from '@/i18n/I18nProvider';
import { clock, duration } from './format';
import { failureKey } from './labels';
import { RunChip } from './StatusChip';
import type { Remote } from './useRemote';
import type { RunSummary } from './types';
import styles from './Mobile.module.css';

/** The current time on the computer's clock, refreshed while something is running. */
export function useNow(offset: number, live: boolean): number {
  const [now, setNow] = useState(() => Date.now() + offset);
  useEffect(() => {
    if (!live) return undefined;
    const timer = setInterval(() => {
      setNow(Date.now() + offset);
    }, 1000);
    return () => {
      clearInterval(timer);
    };
  }, [offset, live]);
  return now;
}

function isFinal(run: RunSummary): boolean {
  return ['completed', 'failed', 'cancelled'].includes(run.status);
}

function RunCard({ run, now }: { run: RunSummary; now: number }) {
  const { t, language } = useI18n();
  const elapsed = duration((run.completedAt ?? now) - run.startedAt);
  return (
    <li>
      <a
        className={styles.card}
        href={`#/run/${encodeURIComponent(run.id)}`}
        data-attention={run.attention}
      >
        <span className={styles.cardTop}>
          <span className={styles.cardTitle}>{run.workflowName}</span>
          <RunChip status={run.status} />
        </span>
        <span className={styles.cardMeta}>{run.workspaceName}</span>
        <span className={styles.cardTask}>{run.task}</span>
        {run.attention === 'needs_answer' && (
          <span className={styles.callout} data-tone="attention">
            {run.pending === 1
              ? t('mobile.card.waitingOne')
              : t('mobile.card.waitingMany', { count: run.pending })}
          </span>
        )}
        {run.attention === 'needs_computer' && (
          <span className={styles.callout} data-tone="attention">
            {t('mobile.card.needsComputer')}
          </span>
        )}
        {run.attention === 'failed' && run.failure && (
          <span className={styles.callout} data-tone="bad">
            {t(failureKey(run.failure.code), {
              step: run.failure.step ?? '',
            })}
          </span>
        )}
        {run.current.length > 0 && (
          <span className={styles.cardCurrent}>
            {run.current.map((step) => (
              <span key={step.nodeId}>
                {step.label}
                {step.agent ? ` · ${step.agent}` : ''}
              </span>
            ))}
          </span>
        )}
        <span className={styles.cardFoot}>
          <span>
            {run.stepsTotal > 0 &&
              t('mobile.card.steps', { done: run.stepsDone, total: run.stepsTotal })}
          </span>
          <span>
            {isFinal(run)
              ? `${clock(run.completedAt ?? run.updatedAt, language, now)} · ${elapsed}`
              : elapsed}
          </span>
        </span>
      </a>
    </li>
  );
}

function Section({
  title,
  runs,
  now,
  tone,
}: {
  title: string;
  runs: RunSummary[];
  now: number;
  tone?: 'attention' | 'bad';
}) {
  if (runs.length === 0) return null;
  return (
    <section className={styles.section} data-tone={tone}>
      <h2 className={styles.sectionTitle}>
        {title} <span className={styles.count}>{runs.length}</span>
      </h2>
      <ul className={styles.list}>
        {runs.map((run) => (
          <RunCard key={run.id} run={run} now={now} />
        ))}
      </ul>
    </section>
  );
}

/**
 * What matters first: what waits for a person. Then what is going, what failed, and what ended.
 * The sections say what the engine says; nothing here is estimated.
 */
export function Dashboard({ remote }: { remote: Remote }) {
  const { t } = useI18n();
  const anyActive = remote.runs.some((run) => !isFinal(run));
  const now = useNow(remote.clockOffset, anyActive);
  const needs = remote.runs.filter(
    (r) => r.attention === 'needs_answer' || r.attention === 'needs_computer',
  );
  const failed = remote.runs.filter((r) => r.attention === 'failed');
  const rest = remote.runs.filter((r) => r.attention === 'none');
  const running = rest.filter((r) => !isFinal(r));
  const finished = rest.filter(isFinal);

  return (
    <div>
      {remote.runs.length === 0 ? (
        <p className={styles.empty}>{t('mobile.dashboard.empty')}</p>
      ) : (
        <>
          <Section title={t('mobile.dashboard.needs')} runs={needs} now={now} tone="attention" />
          <Section title={t('mobile.dashboard.running')} runs={running} now={now} />
          <Section title={t('mobile.dashboard.failed')} runs={failed} now={now} tone="bad" />
          <Section title={t('mobile.dashboard.finished')} runs={finished} now={now} />
        </>
      )}
      <div className={styles.footer}>
        <button
          type="button"
          className={styles.quiet}
          onClick={() => {
            void remote.signOut();
          }}
        >
          {t('mobile.signOut')}
        </button>
        <p className={styles.fine}>{t('mobile.footer')}</p>
      </div>
    </div>
  );
}
