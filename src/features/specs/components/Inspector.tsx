import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { agoText } from '@/features/workflow/model/time';
import { useI18n } from '@/i18n/I18nProvider';
import { failureText, issueText, stageName } from '../model/messages';
import type { SpecArtifact, SpecView, ValidationIssue } from '../types';
import { PanelSection } from './PanelSection';
import styles from './Specs.module.css';

interface Props {
  view: SpecView;
  artifact: SpecArtifact;
  busy: boolean;
  onRevalidate: () => void;
  onFindings: () => void;
}

const ORDER = ['error', 'warning', 'suggestion'] as const;

/** What the checks found in this document, what the review said about it, and how it got here. */
export function Inspector({ view, artifact, busy, onRevalidate, onFindings }: Props) {
  const { t, language } = useI18n();
  const latest = artifact.revisions.at(-1);
  const validation =
    artifact.validation && artifact.validation.revision === latest?.number
      ? artifact.validation
      : null;
  const issues: ValidationIssue[] = validation?.issues ?? [];
  const errors = issues.filter((i) => i.severity === 'error').length;
  const findings = view.spec.findings.filter(
    (f) => f.artifactId === artifact.id && f.status === 'open',
  );
  const runs = view.spec.runs
    .filter((r) => r.artifactId === artifact.id)
    .slice(-5)
    .reverse();
  return (
    <div className={styles.inspectorBody}>
      <PanelSection
        title={t('specs.inspector.checks')}
        tone={errors > 0 ? 'danger' : 'success'}
        {...(validation
          ? {
              tag:
                errors > 0
                  ? t('specs.inspector.errors', { count: errors })
                  : t('specs.inspector.passed'),
            }
          : {})}
      >
        {!latest && <p className={styles.muted}>{t('specs.inspector.nothing')}</p>}
        {latest && !validation && <p className={styles.muted}>{t('specs.inspector.notChecked')}</p>}
        {validation && issues.length === 0 && (
          <p className={styles.okLine}>
            <Icon name="check" size={14} /> {t('specs.inspector.clean')}
          </p>
        )}
        {ORDER.map((severity) => {
          const group = issues.filter((i) => i.severity === severity);
          if (group.length === 0) return null;
          return (
            <div key={severity} className={styles.issueGroup} data-severity={severity}>
              <h4>{t(`specs.inspector.severity.${severity}`, { count: group.length })}</h4>
              <ul>
                {group.map((issue, index) => (
                  <li
                    key={`${issue.code}-${issue.file ?? ''}-${String(issue.line ?? '')}-${String(index)}`}
                  >
                    <span>{issueText(t, issue)}</span>
                    {(issue.file !== null || issue.line !== null) && (
                      <span className={styles.mono}>
                        {issue.file ?? ''}
                        {issue.line ? `:${String(issue.line)}` : ''}
                      </span>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          );
        })}
        {latest && (
          <Button variant="ghost" disabled={busy} onClick={onRevalidate}>
            {t('specs.inspector.rerun')}
          </Button>
        )}
      </PanelSection>

      {findings.length > 0 && (
        <PanelSection
          title={t('specs.inspector.findings')}
          tone="warning"
          tag={String(findings.length)}
        >
          <ul className={styles.plain}>
            {findings.map((finding) => (
              <li key={finding.id}>
                <strong>{finding.title}</strong>
                <span className={styles.muted}>{finding.description}</span>
              </li>
            ))}
          </ul>
          <Button variant="ghost" onClick={onFindings}>
            {t('specs.inspector.seeReview')}
          </Button>
        </PanelSection>
      )}

      {artifact.feedback.length > 0 && (
        <PanelSection title={t('specs.inspector.feedback')}>
          <ul className={styles.plain}>
            {[...artifact.feedback].reverse().map((feedback) => (
              <li key={feedback.id}>
                <span className={styles.feedbackKind}>
                  {t(`specs.feedback.${feedback.kind}`)} ·{' '}
                  {t('specs.doc.revision', { number: feedback.revision })}
                </span>
                {feedback.note && <span>{feedback.note}</span>}
                <span className={styles.muted}>{agoText(feedback.at, language)}</span>
              </li>
            ))}
          </ul>
        </PanelSection>
      )}

      <PanelSection title={t('specs.inspector.history')}>
        {artifact.revisions.length === 0 && (
          <p className={styles.muted}>{t('specs.inspector.nothing')}</p>
        )}
        <ul className={styles.plain}>
          {[...artifact.revisions].reverse().map((revision) => (
            <li key={revision.number}>
              <span>
                {t('specs.doc.revision', { number: revision.number })} ·{' '}
                {t(`specs.origin.${revision.origin}`)}
                {artifact.approvedRevision === revision.number
                  ? ` · ${t('specs.doc.approvedRevision')}`
                  : ''}
              </span>
              <span className={styles.muted}>{agoText(revision.createdAt, language)}</span>
            </li>
          ))}
        </ul>
        {artifact.outdated && (
          <p className={styles.warnLine}>
            <Icon name="warning" size={14} />
            {t('specs.inspector.outdated', { stage: stageName(t, artifact.outdated.because) })}
          </p>
        )}
        {runs.length > 0 && (
          <>
            <h4>{t('specs.inspector.runs')}</h4>
            <ul className={styles.plain}>
              {runs.map((run) => (
                <li key={run.id}>
                  <span className={styles.mono}>{run.id}</span>
                  <span className={styles.muted}>
                    {run.outcome
                      ? t(`specs.runOutcome.${run.outcome}`)
                      : t('specs.runOutcome.running')}
                  </span>
                </li>
              ))}
            </ul>
          </>
        )}
        {artifact.generation.state === 'failed' && (
          <p className={styles.errorLine}>{failureText(t, artifact.generation.message).text}</p>
        )}
      </PanelSection>
    </div>
  );
}
