import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { useCatalog } from '@/features/agents/hooks/useCatalog';
import { useT } from '@/i18n/I18nProvider';
import { errorDetail, errorMessage } from '@/i18n/messages';
import type { TranslationKey } from '@/i18n';
import type {
  HarnessSummaryDto,
  InitModeDto,
  InitializeOutcomeDto,
  ProjectAnalysisDto,
  SemanticRequestDto,
} from '@/lib/tauri/commands';
import { analyzeProject, initializeProject } from '@/features/workspace/services/workspaceService';
import {
  architectureFindings,
  buildInput,
  canConfirm,
  detectedStack,
  initialReview,
  isEditableValue,
  repositoryFindings,
  reviewGroups,
  type ReviewState,
} from '../model/review';
import { AgentAnalysisScreen } from './AgentAnalysisScreen';
import { ConflictsPanel } from './ConflictsPanel';
import { DiffView } from './DiffView';
import { ConfidenceTag, FindingRow, findingName } from './FindingRow';
import { ErrorBox, type Problem } from './ErrorBox';
import { HealthBadge } from './HealthBadge';
import styles from './Harness.module.css';

interface Props {
  workspaceId: string;
  onClose: () => void;
  /** Called with the Harness state once it was created, updated or confirmed. */
  onInitialized: (summary: HarnessSummaryDto) => void;
}

type Stage =
  | { name: 'analyzing' }
  | { name: 'failed'; problem: Problem }
  | { name: 'findings'; analysis: ProjectAnalysisDto }
  | { name: 'review'; analysis: ProjectAnalysisDto; mode: InitModeDto; review: ReviewState }
  | { name: 'done'; outcome: InitializeOutcomeDto };

function problemOf(t: ReturnType<typeof useT>, error: unknown): Problem {
  return { message: errorMessage(t, error), detail: errorDetail(error) };
}

/**
 * Initialize Project: analyze, (optionally) semantic analysis, review, confirm. Nothing is written
 * until the user confirms the review, a model only ever proposes, and an existing Harness is never
 * replaced without their choice (and a backup).
 */
export function InitializeProjectModal({ workspaceId, onClose, onInitialized }: Props) {
  const t = useT();
  const [stage, setStage] = useState<Stage>({ name: 'analyzing' });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<Problem | null>(null);
  const [running, setRunning] = useState<{ agentId: string; agentName: string } | null>(null);

  useEffect(() => {
    let cancelled = false;
    analyzeProject(workspaceId)
      .then((analysis) => {
        if (!cancelled) setStage({ name: 'findings', analysis });
      })
      .catch((e: unknown) => {
        if (!cancelled) setStage({ name: 'failed', problem: problemOf(t, e) });
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceId, t]);

  function runSemantic(request: SemanticRequestDto, agentName: string) {
    setBusy(true);
    setError(null);
    setRunning({ agentId: request.agentId, agentName });
    analyzeProject(workspaceId, request)
      .then((analysis) => {
        setStage({ name: 'findings', analysis });
      })
      .catch((e: unknown) => {
        setError(problemOf(t, e));
      })
      .finally(() => {
        setBusy(false);
        setRunning(null);
      });
  }

  function submit(analysis: ProjectAnalysisDto, mode: InitModeDto, review: ReviewState) {
    setBusy(true);
    setError(null);
    initializeProject(workspaceId, buildInput(analysis, review, mode))
      .then((outcome) => {
        onInitialized(outcome.summary);
        setStage({ name: 'done', outcome });
      })
      .catch((e: unknown) => {
        setError(problemOf(t, e));
      })
      .finally(() => {
        setBusy(false);
      });
  }

  return (
    <>
      {running && (
        <AgentAnalysisScreen
          workspaceId={workspaceId}
          agentId={running.agentId}
          agentName={running.agentName}
        />
      )}
      <Modal label={t('harness.modal.title')} onClose={onClose}>
        <div className={styles.modal}>
          {stage.name === 'analyzing' && <p className={styles.muted}>{t('harness.analyzing')}</p>}
          {stage.name === 'failed' && (
            <>
              <ErrorBox problem={stage.problem} />
              <div className={styles.actions}>
                <Button onClick={onClose}>{t('common.close')}</Button>
              </div>
            </>
          )}
          {stage.name === 'findings' && (
            <FindingsStep
              analysis={stage.analysis}
              busy={busy}
              error={error}
              onCancel={onClose}
              onSemantic={runSemantic}
              onReview={(mode) => {
                setError(null);
                setStage({
                  name: 'review',
                  analysis: stage.analysis,
                  mode,
                  review: initialReview(stage.analysis),
                });
              }}
              onUseExisting={() => {
                submit(stage.analysis, 'use_existing', initialReview(stage.analysis));
              }}
            />
          )}
          {stage.name === 'review' && (
            <ReviewStep
              analysis={stage.analysis}
              review={stage.review}
              busy={busy}
              error={error}
              onChange={(review) => {
                setStage({ ...stage, review });
              }}
              onBack={() => {
                setError(null);
                setStage({ name: 'findings', analysis: stage.analysis });
              }}
              onSubmit={() => {
                submit(stage.analysis, stage.mode, stage.review);
              }}
            />
          )}
          {stage.name === 'done' && <DoneStep outcome={stage.outcome} onClose={onClose} />}
        </div>
      </Modal>
    </>
  );
}

function DoneStep({ outcome, onClose }: { outcome: InitializeOutcomeDto; onClose: () => void }) {
  const t = useT();
  const { summary } = outcome;
  return (
    <>
      <h2 className={styles.title}>{t('harness.done.title')}</h2>
      <p>
        <span className={styles.ok}>✓</span> {t('harness.status.initialized')}{' '}
        {summary.health && <HealthBadge health={summary.health} />}
      </p>
      {summary.health && summary.health.reasons.length > 0 && (
        <ul className={styles.list}>
          {summary.health.reasons.map((reason) => (
            <li key={reason} className={styles.muted}>
              {t(`harness.health.reason.${reason}` as TranslationKey)}
            </li>
          ))}
        </ul>
      )}
      <p>{t('harness.agentsCanUse')}</p>
      <p className={styles.muted}>{t('harness.done.where')}</p>
      {outcome.backedUp.length > 0 && (
        <p className={styles.muted}>
          {t('harness.done.backedUp', { count: outcome.backedUp.length })}
        </p>
      )}
      {outcome.leftUntouched.length > 0 && (
        <p className={styles.warning}>
          {t('harness.done.untouched', { files: outcome.leftUntouched.join(', ') })}
        </p>
      )}
      <div className={styles.actions}>
        <Button onClick={onClose}>{t('common.close')}</Button>
      </div>
    </>
  );
}

interface FindingsProps {
  analysis: ProjectAnalysisDto;
  busy: boolean;
  error: Problem | null;
  onCancel: () => void;
  onReview: (mode: InitModeDto) => void;
  onUseExisting: () => void;
  onSemantic: (request: SemanticRequestDto, agentName: string) => void;
}

function FindingsStep({
  analysis,
  busy,
  error,
  onCancel,
  onReview,
  onUseExisting,
  onSemantic,
}: FindingsProps) {
  const t = useT();
  const stack = detectedStack(analysis);
  const architecture = architectureFindings(analysis);
  const repository = repositoryFindings(analysis);
  const existing = analysis.existing;
  const hasHarness = existing?.status === 'initialized';
  const broken = existing?.status === 'needs_review';

  return (
    <>
      <h2 className={styles.title}>{t('harness.analyze.title')}</h2>
      <p className={styles.muted}>{t('harness.analyze.readOnlyNote')}</p>
      {analysis.partial && (
        <p role="status" className={styles.warning}>
          {t('harness.partial')}
        </p>
      )}

      <SemanticSection analysis={analysis} busy={busy} onRun={onSemantic} />

      <section aria-label={t('harness.detected')}>
        <h3 className={styles.heading}>{t('harness.detected')}</h3>
        {stack.length === 0 ? (
          <p className={styles.muted}>{t('harness.nothingDetected')}</p>
        ) : (
          <ul className={styles.list}>
            {stack.map((finding) => (
              <li key={finding.id}>
                <span className={styles.ok}>✓</span> {finding.label}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section aria-label={t('harness.architecture')}>
        <h3 className={styles.heading}>{t('harness.architecture')}</h3>
        {architecture.length === 0 ? (
          <p className={styles.muted}>{t('harness.architecture.unknown')}</p>
        ) : (
          <ul className={styles.list}>
            {architecture.map((finding) => (
              <li key={finding.id}>
                <span className={styles.maybe}>○</span> {findingName(t, finding)}{' '}
                <ConfidenceTag finding={finding} />
              </li>
            ))}
          </ul>
        )}
      </section>

      <section aria-label={t('harness.repository')}>
        <h3 className={styles.heading}>{t('harness.repository')}</h3>
        <ul className={styles.list}>
          {repository.map((finding) => (
            <li key={finding.id}>
              <span className={styles.ok}>✓</span>{' '}
              {finding.category === 'environment' ? t('harness.envNote') : finding.label}
            </li>
          ))}
        </ul>
      </section>

      <ConflictsPanel conflicts={analysis.conflicts} />
      {analysis.diff && <DiffView diff={analysis.diff} />}

      {(hasHarness || broken || existing?.hasAtlasDir) && (
        <section className={styles.existing} aria-label={t('harness.existing.title')}>
          <h3 className={styles.heading}>{t('harness.existing.title')}</h3>
          <p>
            {hasHarness
              ? t('harness.existing.body')
              : broken
                ? t('harness.existing.needsReview')
                : t('harness.existing.dirOnly')}{' '}
            {existing.health && <HealthBadge health={existing.health} />}
          </p>
        </section>
      )}

      {error && <ErrorBox problem={error} />}
      <div className={styles.actions}>
        <Button onClick={onCancel}>{t('common.cancel')}</Button>
        {hasHarness ? (
          <>
            <Button disabled={busy} onClick={onUseExisting}>
              {t('harness.existing.useExisting')}
            </Button>
            <Button
              disabled={busy}
              onClick={() => {
                onReview('update_existing');
              }}
            >
              {t('harness.existing.update')}
            </Button>
          </>
        ) : (
          <Button
            disabled={busy}
            onClick={() => {
              onReview(broken ? 'update_existing' : 'create');
            }}
          >
            {t('harness.reviewButton')}
          </Button>
        )}
      </div>
    </>
  );
}

/**
 * The recommended step: an agent reads the project (with its own read tools, or, when
 * restricted, only a selected set of evidence) and proposes findings with evidence, plus a draft
 * of the business context. The user is told what leaves their machine before choosing.
 */
function SemanticSection({
  analysis,
  busy,
  onRun,
}: {
  analysis: ProjectAnalysisDto;
  busy: boolean;
  onRun: (request: SemanticRequestDto, agentName: string) => void;
}) {
  const t = useT();
  const { catalog, runtimes } = useCatalog();
  const [agentId, setAgentId] = useState('');
  const [instructions, setInstructions] = useState('');
  const [restricted, setRestricted] = useState(false);
  const capable = new Set(
    (runtimes.status === 'ready' ? runtimes.runtimes : [])
      .filter((r) => r.runtime.capabilities.textOnly)
      .map((r) => r.runtime.id),
  );
  const all = catalog.status === 'ready' ? catalog.agents : [];
  const canRestrict = all.some((a) => capable.has(a.runtimeId));
  const agents = restricted ? all.filter((a) => capable.has(a.runtimeId)) : all;
  const chosen = agents.some((a) => a.id === agentId) ? agentId : (agents[0]?.id ?? '');
  const report = analysis.analysis.semantic;

  return (
    <section aria-label={t('harness.semantic.title')}>
      <h3 className={styles.heading}>{t('harness.semantic.title')}</h3>
      <p>{t('harness.semantic.body')}</p>
      <p className={styles.muted}>
        {restricted ? t('harness.semantic.disclosureRestricted') : t('harness.semantic.disclosure')}
      </p>
      {all.length === 0 ? (
        <p className={styles.muted}>{t('harness.semantic.noAgents')}</p>
      ) : (
        <>
          <label className={styles.field}>
            <span>{t('harness.semantic.instructions')}</span>
            <textarea
              className={styles.control}
              rows={3}
              value={instructions}
              placeholder={t('harness.semantic.instructionsPlaceholder')}
              onChange={(event) => {
                setInstructions(event.target.value);
              }}
            />
          </label>
          {canRestrict && (
            <label className={styles.check}>
              <input
                type="checkbox"
                checked={restricted}
                onChange={(event) => {
                  setRestricted(event.target.checked);
                }}
              />
              <span>{t('harness.semantic.restricted')}</span>
            </label>
          )}
          <div className={styles.row}>
            <label className={styles.check}>
              <span>{t('harness.semantic.agent')}</span>
              <select
                className={styles.value}
                value={chosen}
                onChange={(event) => {
                  setAgentId(event.target.value);
                }}
              >
                {agents.map((agent) => (
                  <option key={agent.id} value={agent.id}>
                    {agent.name}
                  </option>
                ))}
              </select>
            </label>
            <Button
              disabled={busy || !chosen}
              onClick={() => {
                onRun(
                  { agentId: chosen, instructions: instructions.trim(), restricted },
                  agents.find((a) => a.id === chosen)?.name ?? chosen,
                );
              }}
            >
              {busy ? t('harness.semantic.running') : t('harness.semantic.run')}
            </Button>
          </div>
          {busy && <p className={styles.muted}>{t('harness.semantic.wait')}</p>}
        </>
      )}
      {report.status === 'completed' && (
        <div role="status">
          <p>{t('harness.semantic.completed')}</p>
          {report.explored ? (
            <p className={styles.muted}>{t('harness.semantic.explored')}</p>
          ) : (
            <>
              <p className={styles.muted}>{t('harness.semantic.sent')}</p>
              <ul className={styles.sources}>
                {report.sentFiles.map((file) => (
                  <li key={file}>
                    <code>{file}</code>
                  </li>
                ))}
              </ul>
            </>
          )}
          {report.rejected > 0 && (
            <p className={styles.muted}>
              {t('harness.semantic.rejected', { count: report.rejected })}
            </p>
          )}
        </div>
      )}
      {report.status === 'failed' && (
        <ErrorBox
          problem={{
            message: t('harness.semantic.failed', {
              message: t(`error.${report.error ?? 'semantic_analysis_failed'}` as TranslationKey),
            }),
            detail: report.errorDetail ?? undefined,
          }}
        />
      )}
    </section>
  );
}

interface ReviewProps {
  analysis: ProjectAnalysisDto;
  review: ReviewState;
  busy: boolean;
  error: Problem | null;
  onChange: (review: ReviewState) => void;
  onBack: () => void;
  onSubmit: () => void;
}

const USER_FIELDS = [
  ['purpose', 'harness.review.purpose', 'input'],
  ['users', 'harness.review.users', 'input'],
  ['concepts', 'harness.review.concepts', 'textarea'],
  ['businessRules', 'harness.review.rules', 'textarea'],
  ['constraints', 'harness.review.constraints', 'textarea'],
  ['decisions', 'harness.review.decisions', 'textarea'],
] as const;

function ReviewStep({ analysis, review, busy, error, onChange, onBack, onSubmit }: ReviewProps) {
  const t = useT();
  const groups = reviewGroups(analysis);

  const toggle = (id: string, list: 'excluded' | 'confirmed') => {
    const current = review[list];
    onChange({
      ...review,
      [list]: current.includes(id) ? current.filter((x) => x !== id) : [...current, id],
    });
  };

  return (
    <form
      className={styles.form}
      aria-label={t('harness.review.title')}
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
    >
      <h2 className={styles.title}>{t('harness.review.title')}</h2>

      <ConflictsPanel
        conflicts={analysis.conflicts}
        decisions={review.values}
        onChoose={(id, value) => {
          onChange({ ...review, values: { ...review.values, [id]: value } });
        }}
      />

      {groups.map((group) => (
        <fieldset key={group.id} className={styles.group}>
          <legend className={styles.heading}>
            {t(`harness.review.groups.${group.id}` as TranslationKey)}
          </legend>
          {group.findings.map((finding) => (
            <FindingRow
              key={finding.id}
              finding={finding}
              included={!review.excluded.includes(finding.id)}
              onToggle={() => {
                toggle(finding.id, 'excluded');
              }}
              confirmed={review.confirmed.includes(finding.id)}
              onConfirm={
                canConfirm(finding)
                  ? () => {
                      toggle(finding.id, 'confirmed');
                    }
                  : undefined
              }
              value={review.values[finding.id] ?? finding.value}
              onValue={
                isEditableValue(finding)
                  ? (value) => {
                      onChange({ ...review, values: { ...review.values, [finding.id]: value } });
                    }
                  : undefined
              }
            />
          ))}
        </fieldset>
      ))}
      {groups.every((g) => g.id !== 'architecture') && (
        <p className={styles.muted}>{t('harness.architecture.unknown')}</p>
      )}

      <fieldset className={styles.group}>
        <legend className={styles.heading}>{t('harness.review.business')}</legend>
        <p className={styles.muted}>{t('harness.review.optional')}</p>
        {USER_FIELDS.map(([field, label, kind]) => (
          <label key={field} className={styles.field}>
            <span>{t(label)}</span>
            {review.suggested.includes(field) && (
              <span className={styles.muted}>{t('harness.review.suggested')}</span>
            )}
            {kind === 'input' ? (
              <input
                className={styles.control}
                value={review[field]}
                onChange={(event) => {
                  onChange({ ...review, [field]: event.target.value });
                }}
              />
            ) : (
              <textarea
                className={styles.control}
                rows={3}
                value={review[field]}
                onChange={(event) => {
                  onChange({ ...review, [field]: event.target.value });
                }}
              />
            )}
          </label>
        ))}
        <p className={styles.muted}>{t('harness.review.keepNote')}</p>
        <p className={styles.muted}>{t('harness.review.secretsNote')}</p>
        {analysis.unmanagedUserFiles.length > 0 && (
          <p role="status" className={styles.warning}>
            {t('harness.review.unmanaged', { files: analysis.unmanagedUserFiles.join(', ') })}
          </p>
        )}
      </fieldset>

      {error && <ErrorBox problem={error} />}
      <div className={styles.actions}>
        <Button onClick={onBack}>{t('harness.back')}</Button>
        <Button type="submit" disabled={busy}>
          {busy ? t('harness.initializing') : t('harness.initializeButton')}
        </Button>
      </div>
    </form>
  );
}
