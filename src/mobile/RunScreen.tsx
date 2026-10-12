import { useState } from 'react';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import { api, ApiError } from './api';
import { APPLIED_KEY, InteractionCard } from './InteractionCard';
import { useNow } from './Dashboard';
import { clock, duration } from './format';
import { failureKey } from './labels';
import { RunChip } from './StatusChip';
import { Timeline } from './Timeline';
import { useRunDetail, type Remote } from './useRemote';
import type { Applied, RunDetail } from './types';
import styles from './Mobile.module.css';

const CONTROL_PROBLEM: Record<string, TranslationKey> = {
  run_not_live: 'mobile.control.notLive',
  state_invalid: 'mobile.control.invalid',
  desktop_only: 'mobile.control.desktopOnly',
  rate_limited: 'mobile.answer.rateLimited',
  unreachable: 'mobile.answer.unreachable',
};

function Controls({ run, onDone }: { run: RunDetail; onDone: (run: RunDetail) => void }) {
  const { t } = useI18n();
  const [busy, setBusy] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [problem, setProblem] = useState<TranslationKey | null>(null);

  const act = async (action: 'pause' | 'resume' | 'cancel') => {
    setBusy(true);
    setProblem(null);
    try {
      onDone(await api.control(run.id, action, action === 'cancel'));
      setConfirming(false);
    } catch (error) {
      const code = error instanceof ApiError ? error.code : 'unreachable';
      setProblem(CONTROL_PROBLEM[code] ?? 'mobile.control.failed');
    } finally {
      setBusy(false);
    }
  };

  if (!run.controls.pause && !run.controls.resume && !run.controls.cancel) return null;
  return (
    <section className={styles.panel} aria-label={t('mobile.control.title')}>
      <h2 className={styles.panelTitle}>{t('mobile.control.title')}</h2>
      <div className={styles.choices}>
        {run.controls.pause && (
          <button
            type="button"
            className={styles.secondary}
            disabled={busy}
            onClick={() => {
              void act('pause');
            }}
          >
            {t('mobile.control.pause')}
          </button>
        )}
        {run.controls.resume && (
          <button
            type="button"
            className={styles.primary}
            disabled={busy}
            onClick={() => {
              void act('resume');
            }}
          >
            {t('mobile.control.resume')}
          </button>
        )}
        {run.controls.cancel && !confirming && (
          <button
            type="button"
            className={styles.secondary}
            data-decline="true"
            onClick={() => {
              setConfirming(true);
            }}
          >
            {t('mobile.control.cancel')}
          </button>
        )}
      </div>
      {confirming && (
        <div
          className={styles.confirm}
          role="alertdialog"
          aria-label={t('mobile.control.confirmTitle')}
        >
          <p>{t('mobile.control.confirmText')}</p>
          <div className={styles.choices}>
            <button
              type="button"
              className={styles.danger}
              disabled={busy}
              onClick={() => {
                void act('cancel');
              }}
            >
              {t('mobile.control.confirmYes')}
            </button>
            <button
              type="button"
              className={styles.secondary}
              disabled={busy}
              onClick={() => {
                setConfirming(false);
              }}
            >
              {t('mobile.control.confirmNo')}
            </button>
          </div>
        </div>
      )}
      {problem && (
        <p className={styles.error} role="alert">
          {t(problem)}
        </p>
      )}
    </section>
  );
}

/** Everything about one run that is useful away from the computer, most urgent first. */
export function RunScreen({ remote, id }: { remote: Remote; id: string }) {
  const { t, language } = useI18n();
  const { run, problem, reload, adopt } = useRunDetail(remote, id);
  // What the computer said about the last answer. It outlives the question, which becomes history.
  const [confirmed, setConfirmed] = useState<Applied | null>(null);
  const settle = (next: RunDetail, applied: Applied) => {
    setConfirmed(applied);
    adopt(next);
  };
  const active = run ? !['completed', 'failed', 'cancelled'].includes(run.status) : false;
  const now = useNow(remote.clockOffset, active);

  if (problem === 'not_found') {
    return <p className={styles.empty}>{t('mobile.run.notFound')}</p>;
  }
  if (!run) {
    return (
      <p className={styles.empty} role="status">
        {problem === 'failed' ? t('mobile.run.failed') : t('mobile.loading')}
      </p>
    );
  }

  const pending = run.interactions.filter((i) => i.status === 'pending');
  const settled = run.interactions.filter((i) => i.status !== 'pending');
  const elapsed = duration((run.completedAt ?? now) - run.startedAt);

  return (
    <article className={styles.run}>
      <header className={styles.runHead}>
        <h1 className={styles.runTitle}>{run.workflowName}</h1>
        <RunChip status={run.status} />
      </header>
      <p className={styles.muted}>
        {run.workspaceName} ·{' '}
        {t('mobile.run.started', { time: clock(run.startedAt, language, now) })} · {elapsed}
      </p>
      <p className={styles.task}>{run.task}</p>

      {run.stepsTotal > 0 && (
        <p className={styles.fine}>
          {t('mobile.card.steps', { done: run.stepsDone, total: run.stepsTotal })}
        </p>
      )}

      {run.status === 'interrupted' && (
        <p className={styles.callout} data-tone="attention" role="status">
          {t('mobile.run.interrupted')}
        </p>
      )}
      {run.steps.some((s) => s.status === 'waiting_approval') && (
        <p className={styles.callout} data-tone="attention" role="status">
          {t('mobile.run.commandApproval')}
        </p>
      )}
      {run.failure && (
        <p className={styles.callout} data-tone="bad" role="note">
          {t(failureKey(run.failure.code), { step: run.failure.step ?? '' })}
          {run.failure.detail ? ` — ${run.failure.detail}` : ''}
          {' · '}
          {t('mobile.run.recoverOnComputer')}
        </p>
      )}

      {confirmed && (
        <p className={styles.success} role="status">
          {t(APPLIED_KEY[confirmed])}
        </p>
      )}

      {pending.map((interaction) => (
        <InteractionCard
          key={interaction.id}
          run={run}
          interaction={interaction}
          onSettled={settle}
          onStale={() => {
            void reload();
          }}
        />
      ))}

      <Controls run={run} onDone={adopt} />

      <section className={styles.panel}>
        <h2 className={styles.panelTitle}>{t('mobile.run.timeline')}</h2>
        <Timeline steps={run.steps} edges={run.edges} now={now} />
      </section>

      {settled.length > 0 && (
        <section className={styles.panel}>
          <h2 className={styles.panelTitle}>{t('mobile.run.history')}</h2>
          {settled.map((interaction) => (
            <InteractionCard
              key={interaction.id}
              run={run}
              interaction={interaction}
              onSettled={settle}
              onStale={() => {
                void reload();
              }}
            />
          ))}
        </section>
      )}

      {(Boolean(run.result.summary) ||
        run.result.artifacts.length > 0 ||
        run.result.decisions.length > 0) && (
        <section className={styles.panel}>
          <h2 className={styles.panelTitle}>
            {run.status === 'completed' ? t('mobile.run.result') : t('mobile.run.sofar')}
          </h2>
          {run.result.summary && <p className={styles.pre}>{run.result.summary}</p>}
          {run.result.artifacts.length > 0 && (
            <ul className={styles.bullets}>
              {run.result.artifacts.map((artifact) => (
                <li key={artifact.name}>
                  <strong>{artifact.name}</strong>
                  {artifact.summary ? ` — ${artifact.summary}` : ''}
                </li>
              ))}
            </ul>
          )}
          {run.result.decisions.length > 0 && (
            <ul className={styles.bullets}>
              {run.result.decisions.map((decision) => (
                <li key={decision.title}>
                  <strong>{decision.title}</strong> — {decision.decision}
                </li>
              ))}
            </ul>
          )}
        </section>
      )}

      {run.code.filesChanged > 0 && (
        <section className={styles.panel}>
          <h2 className={styles.panelTitle}>{t('mobile.run.code')}</h2>
          <p>
            {t('mobile.run.codeSummary', {
              files: run.code.filesChanged,
              added: run.code.additions,
              removed: run.code.deletions,
            })}
          </p>
          <p className={styles.fine}>{t('mobile.run.codeOnComputer')}</p>
        </section>
      )}

      {run.events.length > 0 && (
        <details className={styles.panel}>
          <summary className={styles.panelTitle}>{t('mobile.run.log')}</summary>
          <ol className={styles.log} reversed>
            {[...run.events].reverse().map((event, index) => (
              <li key={`${String(event.at)}-${String(index)}`}>
                <time className={styles.fine}>{clock(event.at, language, now)}</time>{' '}
                {event.step ? <strong>{event.step}: </strong> : null}
                {event.message}
              </li>
            ))}
          </ol>
        </details>
      )}
    </article>
  );
}
