import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { useI18n } from '@/i18n/I18nProvider';
import { agoText } from '@/features/workflow/model/time';
import { blockerText, issueText } from '../model/messages';
import { canApproveFinal, checksOf } from '../model/readiness';
import type { FindingStatus, SpecFinding, SpecQuestion, SpecView, Stage } from '../types';
import { CliPanel } from './CliPanel';
import { PublishPanel } from './PublishPanel';
import { ReviewPanel } from './ReviewPanel';
import { PanelSection } from './PanelSection';
import styles from './Specs.module.css';

interface Props {
  view: SpecView;
  busy: boolean;
  reviewerName: string;
  onGoto: (stage: Stage) => void;
  onRevalidate: () => void;
  onRunReview: () => void;
  onWaive: (note: string) => void;
  onAnswerQuestion: (question: SpecQuestion, choice: string | null, answer: string | null) => void;
  onAnswerFindings: (answers: { findingId: string; answer: string }[]) => Promise<boolean>;
  onResolve: (findingId: string, status: FindingStatus, note: string | null) => void;
  onShowFinding: (finding: SpecFinding) => void;
  onApprove: () => void;
  onPublish: (changeId: string) => void;
  onArchive: () => void;
  onImplement: () => void;
  onCancel: () => void;
  onCliCheck: () => void;
  onCliArchive: () => void;
  /** The last failure of an action, for the panel it belongs to. */
  error: unknown;
}

/**
 * The last stage: what is still in the way of the spec being ready, the review of the whole, the
 * person's last word, and where the spec goes next. "Ready" is never said by an agent: it is
 * what is left when every one of these holds.
 */
export function FinalReview(props: Props) {
  const { view, busy } = props;
  const { t, language } = useI18n();
  const checks = checksOf(view);
  const consistency = view.consistencyCurrent ? view.spec.consistency : null;
  const consistencyIssues = consistency?.issues ?? [];
  const approvable = canApproveFinal(view);
  const approval = view.spec.finalApproval;

  return (
    <div className={styles.final}>
      <section className={styles.panel} aria-label={t('specs.final.readiness')}>
        <header className={styles.readinessHead}>
          <h3>{t('specs.final.readiness')}</h3>
          <Badge dot mono tone={view.readiness.ready ? 'success' : 'warning'}>
            {view.readiness.ready ? t('specs.final.ready') : t('specs.final.notReady')}
          </Badge>
          <Button variant="ghost" disabled={busy} onClick={props.onRevalidate}>
            {t('specs.inspector.rerun')}
          </Button>
          <Button
            className={styles.readinessApprove}
            disabled={!approvable || busy || view.finalApprovalCurrent}
            onClick={props.onApprove}
          >
            {t('specs.final.approve')}
          </Button>
        </header>
        <ul className={styles.checks}>
          {checks.map((check) => (
            <li key={check.id} data-ok={check.ok}>
              <span className={styles.checkMark} aria-hidden="true">
                {check.ok ? <Icon name="check" size={13} /> : <Icon name="clock" size={13} />}
              </span>
              <div>
                <strong>{t(`specs.check.${check.id}`)}</strong>
                {check.ok ? (
                  <span className={styles.muted}>{t(`specs.check.${check.id}.ok`)}</span>
                ) : (
                  <ul className={styles.plain}>
                    {check.blockers.map((blocker, index) => (
                      <li key={`${blocker.code}-${blocker.artifactId ?? ''}-${String(index)}`}>
                        <span>{blockerText(t, blocker)}</span>
                        {blocker.artifactId && (
                          <Button
                            variant="ghost"
                            onClick={() => {
                              props.onGoto(blocker.artifactId as Stage);
                            }}
                          >
                            {t('specs.final.goto')}
                          </Button>
                        )}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            </li>
          ))}
        </ul>
        {approval && view.finalApprovalCurrent && (
          <p className={styles.muted}>
            {t('specs.final.approvedAt', { when: agoText(approval.at, language) })}
            {approval.acknowledgedWarnings.length > 0 &&
              ` · ${t('specs.final.warningsKnown', { count: approval.acknowledgedWarnings.length })}`}
          </p>
        )}
        {!approvable && !view.finalApprovalCurrent && (
          <p className={styles.muted}>{t('specs.final.approveNeeds')}</p>
        )}
      </section>

      {consistencyIssues.length > 0 && (
        <PanelSection title={t('specs.final.consistency')} tone="warning">
          <ul className={styles.plain}>
            {consistencyIssues.map((issue, index) => (
              <li
                key={`${issue.code}-${issue.file ?? ''}-${String(index)}`}
                data-severity={issue.severity}
              >
                <span>{issueText(t, issue)}</span>
                <span className={styles.muted}>
                  {t(`specs.inspector.one.${issue.severity}`)}
                  {issue.file ? ` · ${issue.file}` : ''}
                </span>
              </li>
            ))}
          </ul>
        </PanelSection>
      )}

      <ReviewPanel
        view={view}
        busy={busy}
        reviewerName={props.reviewerName}
        onRun={props.onRunReview}
        onWaive={props.onWaive}
        onAnswerQuestion={props.onAnswerQuestion}
        onAnswerFindings={props.onAnswerFindings}
        onResolve={props.onResolve}
        onShowFinding={props.onShowFinding}
        onCancel={props.onCancel}
      />

      <PublishPanel
        view={view}
        busy={busy}
        onPublish={props.onPublish}
        onArchive={props.onArchive}
      />

      <CliPanel
        view={view}
        busy={busy}
        error={props.error}
        onCheck={props.onCliCheck}
        onArchive={props.onCliArchive}
      />

      <PanelSection title={t('specs.implement.title')} defaultOpen={false}>
        <p className={styles.muted}>{t('specs.implement.intro')}</p>
        <div className={styles.row}>
          <Button
            disabled={!view.readiness.ready || busy || !!view.spec.archived}
            onClick={props.onImplement}
          >
            {t('specs.implement.start')}
          </Button>
        </div>
        {!view.readiness.ready && <p className={styles.muted}>{t('specs.implement.needsReady')}</p>}
        {view.spec.implementations.length > 0 && (
          <ul className={styles.plain}>
            {[...view.spec.implementations].reverse().map((link) => (
              <li key={link.workflowExecutionId}>
                <span>{link.workflowName}</span>
                <span className={styles.muted}>{agoText(link.startedAt, language)}</span>
              </li>
            ))}
          </ul>
        )}
      </PanelSection>
    </div>
  );
}
