import { useEffect, useId, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
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
  unresolvedConflicts,
  type ReviewGroupId,
  shortPath,
  type ReviewState,
} from '../model/review';
import { AgentAnalysisScreen } from './AgentAnalysisScreen';
import { ConflictsPanel } from './ConflictsPanel';
import { DiffView } from './DiffView';
import {
  ConfidenceTag,
  EvidenceDetails,
  FindingRow,
  findingName,
  inspectorName,
} from './FindingRow';
import { ErrorBox, type Problem } from './ErrorBox';
import { HealthBadge } from './HealthBadge';
import styles from './Harness.module.css';
import rv from './Review.module.css';

interface Props {
  workspaceId: string;
  onClose: () => void;
  /** Called with the Harness state once it was created, updated or confirmed. */
  onInitialized: (summary: HarnessSummaryDto) => void;
}

const cx = (...parts: (string | undefined)[]) => parts.filter(Boolean).join(' ');

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
      <Modal
        label={t('harness.modal.title')}
        onClose={onClose}
        size={stage.name === 'review' ? 'wide' : stage.name === 'findings' ? 'analyze' : 'normal'}
      >
        <div className={styles.modal}>
          {stage.name === 'analyzing' && <p className={styles.muted}>{t('harness.analyzing')}</p>}
          {stage.name === 'failed' && (
            <>
              <ErrorBox problem={stage.problem} />
              <div className={styles.actions}>
                <Button variant="secondary" onClick={onClose}>
                  {t('common.close')}
                </Button>
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
      {outcome.gitIgnore !== 'skipped' && (
        <p className={outcome.gitIgnore === 'failed' ? styles.warning : styles.muted} role="status">
          {t(`harness.done.gitIgnore.${outcome.gitIgnore}` as TranslationKey)}
        </p>
      )}
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
        <Button variant="secondary" onClick={onClose}>
          {t('common.close')}
        </Button>
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

/** Rough size of the user's note for the model, shown next to the field. */
const estimateTokens = (text: string) => Math.ceil(text.trim().length / 4);

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
  const ai = useSemanticForm();
  const aiDone = analysis.analysis.semantic.status === 'completed';

  return (
    <>
      <header className={rv.analyzeHead}>
        <div className={rv.titleRow}>
          <h2 className={rv.title}>{t('harness.analyze.title')}</h2>
          <span className={cx(rv.chip, rv.chipAccent)}>
            <Icon name="sparkle" size={13} />
            {t('harness.analyze.engine')}
          </span>
          <span className={cx(rv.chip, rv.chipInfo)}>
            <span className={rv.dot} aria-hidden="true" />
            {t('harness.analyze.localCore')}
          </span>
        </div>
        <p className={rv.lockNote}>
          <Icon name="lock" size={15} />
          {t('harness.analyze.readOnlyNote')}
        </p>
      </header>
      {analysis.partial && (
        <p role="status" className={styles.warning}>
          {t('harness.partial')}
        </p>
      )}

      <SemanticSection analysis={analysis} busy={busy} form={ai} />

      <section className={rv.aiCard}>
        <div className={rv.aiHead}>
          <h3 className={cx(rv.cardTitle)}>
            <Icon name="cpu" size={16} />
            {t('harness.diagnostic.title')}
          </h3>
          <span className={cx(rv.chip, rv.chipInfo)}>
            <span className={rv.dot} aria-hidden="true" />
            {t('harness.diagnostic.chip')}
          </span>
        </div>
        <div className={rv.diagCols}>
          <section className={rv.diagCol} aria-label={t('harness.detected')}>
            <h4 className={rv.diagHead}>
              {t('harness.diagnostic.stack')}
              <Icon name="cpu" size={14} />
            </h4>
            {stack.length === 0 ? (
              <p className={rv.source}>{t('harness.nothingDetected')}</p>
            ) : (
              <ul className={rv.diagList}>
                {stack.map((finding) => (
                  <li key={finding.id}>
                    <span className={rv.tick}>✓</span>
                    {finding.label}
                  </li>
                ))}
              </ul>
            )}
          </section>

          <section className={rv.diagCol} aria-label={t('harness.repository')}>
            <h4 className={rv.diagHead}>
              {t('harness.diagnostic.git')}
              <Icon name="commit" size={14} />
            </h4>
            <ul className={rv.diagList}>
              {repository.map((finding) => (
                <li key={finding.id}>
                  <span className={rv.tick}>✓</span>
                  {finding.category === 'environment' ? t('harness.envNote') : finding.label}
                </li>
              ))}
            </ul>
          </section>

          <section className={rv.diagCol} aria-label={t('harness.architecture')}>
            <h4 className={rv.diagHead}>
              {t('harness.diagnostic.topology')}
              <Icon name="tree" size={14} />
            </h4>
            {architecture.length === 0 ? (
              <p className={rv.source}>{t('harness.architecture.unknown')}</p>
            ) : (
              <ul className={rv.diagList}>
                {architecture.map((finding) => (
                  <li key={finding.id}>
                    <span className={rv.tick}>○</span>
                    {findingName(t, finding)} <ConfidenceTag finding={finding} />
                  </li>
                ))}
              </ul>
            )}
          </section>
        </div>
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
      <footer className={cx(rv.footer, rv.analyzeFooter)}>
        <button type="button" className={rv.textButton} onClick={onCancel}>
          {t('common.cancel')}
        </button>
        <div className={rv.footerActions}>
          {hasHarness ? (
            <>
              <button
                type="button"
                className={rv.ghostButton}
                disabled={busy}
                onClick={onUseExisting}
              >
                {t('harness.existing.useExisting')}
              </button>
              <button
                type="button"
                className={rv.ghostButton}
                disabled={busy}
                onClick={() => {
                  onReview('update_existing');
                }}
              >
                <Icon name="eye" size={15} />
                {t('harness.existing.update')}
              </button>
            </>
          ) : (
            <button
              type="button"
              className={rv.ghostButton}
              disabled={busy}
              onClick={() => {
                onReview(broken ? 'update_existing' : 'create');
              }}
            >
              <Icon name="eye" size={15} />
              {aiDone || !ai.canRun ? t('harness.reviewButton') : t('harness.reviewWithoutAi')}
            </button>
          )}
          {ai.canRun && (
            <button
              type="button"
              className={rv.primaryButton}
              disabled={busy || !ai.chosen}
              onClick={() => {
                onSemantic(
                  {
                    agentId: ai.chosen,
                    instructions: ai.instructions.trim(),
                    restricted: ai.restricted,
                  },
                  ai.agents.find((a) => a.id === ai.chosen)?.name ?? ai.chosen,
                );
              }}
            >
              <Icon name="sparkle" size={15} />
              {busy ? t('harness.semantic.running') : t('harness.semantic.run')}
            </button>
          )}
        </div>
      </footer>
    </>
  );
}

/** What the user chose for the AI analysis; the footer button runs it. */
function useSemanticForm() {
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
  return {
    all,
    agents,
    canRestrict,
    chosen,
    canRun: all.length > 0,
    instructions,
    restricted,
    setAgentId,
    setInstructions,
    setRestricted,
  };
}

/**
 * The recommended step: an agent reads the project (with its own read tools, or, when
 * restricted, only a selected set of evidence) and proposes findings with evidence, plus a draft
 * of the business context. The user is told what leaves their machine before choosing.
 */
function SemanticSection({
  analysis,
  busy,
  form,
}: {
  analysis: ProjectAnalysisDto;
  busy: boolean;
  form: ReturnType<typeof useSemanticForm>;
}) {
  const t = useT();
  const { all, agents, canRestrict, chosen, instructions, restricted } = form;
  const report = analysis.analysis.semantic;
  const agent = agents.find((a) => a.id === chosen);
  const inputId = useId();

  return (
    <section className={rv.aiCard} aria-label={t('harness.semantic.title')}>
      <div className={rv.aiHead}>
        <h3 className={rv.aiTitle}>
          <Icon name="agents" size={16} />
          {t('harness.semantic.title')}
        </h3>
        <span className={rv.pill}>{t('harness.semantic.badge')}</span>
      </div>
      <p className={rv.body}>{t('harness.semantic.body')}</p>
      <p className={rv.body}>
        {restricted ? t('harness.semantic.disclosureRestricted') : t('harness.semantic.disclosure')}
      </p>
      {all.length === 0 ? (
        <p className={rv.body}>{t('harness.semantic.noAgents')}</p>
      ) : (
        <>
          <div className={rv.field}>
            <div className={rv.fieldHead}>
              <label htmlFor={inputId}>{t('harness.semantic.instructions')}</label>
              <span className={rv.counter}>
                <Icon name="database" size={12} />
                {t('harness.semantic.tokens', { count: estimateTokens(instructions) })}
              </span>
            </div>
            <textarea
              id={inputId}
              className={rv.textarea}
              rows={3}
              value={instructions}
              placeholder={t('harness.semantic.instructionsPlaceholder')}
              onChange={(event) => {
                form.setInstructions(event.target.value);
              }}
            />
          </div>
          {canRestrict && (
            <label className={rv.option}>
              <input
                type="checkbox"
                checked={restricted}
                onChange={(event) => {
                  form.setRestricted(event.target.checked);
                }}
              />
              <span className={rv.optionText}>
                <span className={rv.optionTitle}>
                  {t('harness.semantic.restrictedTitle')}
                  <span title={t('harness.semantic.disclosureRestricted')}>
                    <Icon name="help" size={14} />
                  </span>
                </span>
                <span>{t('harness.semantic.restricted')}</span>
              </span>
            </label>
          )}
          <div className={rv.field}>
            <span className={rv.fieldLabel}>{t('harness.semantic.executor')}</span>
            <label className={rv.select}>
              <span className={rv.selectMain}>
                <span className={rv.selectIcon}>
                  <Icon name="agents" size={15} />
                </span>
                <span className={rv.selectText}>
                  <span className={rv.selectName}>{agent?.name ?? ''}</span>
                  <span className={rv.selectSub}>{agent?.modelId ?? ''}</span>
                </span>
              </span>
              <Icon name="unfold" size={16} />
              <select
                aria-label={t('harness.semantic.agent')}
                value={chosen}
                onChange={(event) => {
                  form.setAgentId(event.target.value);
                }}
              >
                {agents.map((a) => (
                  <option key={a.id} value={a.id}>
                    {a.name}
                  </option>
                ))}
              </select>
            </label>
          </div>
          {busy && <p className={rv.body}>{t('harness.semantic.wait')}</p>}
        </>
      )}
      {report.status === 'completed' && (
        <div role="status">
          <p className={rv.body}>{t('harness.semantic.completed')}</p>
          {report.explored ? (
            <p className={rv.body}>{t('harness.semantic.explored')}</p>
          ) : (
            <>
              <p className={rv.body}>{t('harness.semantic.sent')}</p>
              <ul className={rv.diagList}>
                {report.sentFiles.map((file) => (
                  <li key={file}>
                    <code>{file}</code>
                  </li>
                ))}
              </ul>
            </>
          )}
          {report.rejected > 0 && (
            <p className={rv.body}>{t('harness.semantic.rejected', { count: report.rejected })}</p>
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

const USER_FIELDS = [
  ['purpose', 'harness.review.purpose', 'input'],
  ['users', 'harness.review.users', 'input'],
  ['concepts', 'harness.review.concepts', 'textarea'],
  ['businessRules', 'harness.review.rules', 'textarea'],
  ['constraints', 'harness.review.constraints', 'textarea'],
  ['decisions', 'harness.review.decisions', 'textarea'],
] as const;

interface ReviewProps {
  analysis: ProjectAnalysisDto;
  review: ReviewState;
  busy: boolean;
  error: Problem | null;
  onChange: (review: ReviewState) => void;
  onBack: () => void;
  onSubmit: () => void;
}

type TabId = ReviewGroupId | 'business';

function ReviewStep({ analysis, review, busy, error, onChange, onBack, onSubmit }: ReviewProps) {
  const t = useT();
  const groups = reviewGroups(analysis);
  const tabs: TabId[] = [...groups.map((g) => g.id), 'business'];
  const [tab, setTab] = useState<TabId>(tabs[0] ?? 'business');
  const [inspectedId, setInspectedId] = useState<string | null>(null);
  const inspected = analysis.findings.find((f) => f.id === inspectedId) ?? null;
  const current = groups.find((g) => g.id === tab);

  const toggle = (id: string, list: 'excluded' | 'confirmed') => {
    const current = review[list];
    onChange({
      ...review,
      [list]: current.includes(id) ? current.filter((x) => x !== id) : [...current, id],
    });
  };

  const reviewable = groups.flatMap((g) => g.findings);
  const verified = reviewable.filter((f) => f.verification.status === 'verified').length;
  const inferred = reviewable.filter((f) => f.origin === 'inference').length;
  const pending = unresolvedConflicts(analysis, review).length;
  const included = reviewable.filter((f) => !review.excluded.includes(f.id)).length;

  const metrics = [
    {
      label: t('harness.review.stats.total'),
      value: t('harness.review.stats.items', { count: reviewable.length }),
      tag: t('harness.review.stats.included', { count: included }),
      icon: 'layout',
      tone: rv.toneAccent,
    },
    {
      label: t('harness.review.stats.verified'),
      value: t('harness.review.stats.items', { count: verified }),
      tag: 'git ref',
      icon: 'endNode',
      tone: rv.toneInfo,
    },
    {
      label: t('harness.review.stats.inferred'),
      value: t('harness.review.stats.rules', { count: inferred }),
      tag: t('harness.confirmShort'),
      icon: 'agents',
      tone: rv.tonePolicy,
    },
    {
      label: t('harness.review.stats.pending'),
      value: t('harness.review.stats.conflicts', { count: pending }),
      tag: pending > 0 ? '!' : '✓',
      icon: 'shield',
      tone: pending > 0 ? rv.toneDanger : rv.toneNeutral,
    },
  ] as const;

  return (
    <form
      className={rv.workbench}
      aria-label={t('harness.review.title')}
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
    >
      <header className={rv.header}>
        <div className={rv.headText}>
          <div className={rv.titleRow}>
            <h2 className={rv.title}>{t('harness.review.title')}</h2>
            <span className={rv.lockChip}>
              <span className={rv.dot} aria-hidden="true" />
              .atlas/
            </span>
          </div>
          <p className={rv.subtitle}>{t('harness.review.subtitle')}</p>
        </div>
        <div className={rv.headActions}>
          <button type="button" className={rv.ghostButton} onClick={onBack}>
            <Icon name="undo" size={14} />
            {t('harness.back')}
          </button>
          <button type="submit" className={rv.primaryButton} disabled={busy}>
            <Icon name="endNode" size={15} />
            {busy ? t('harness.initializing') : t('harness.initializeButton')}
          </button>
        </div>
      </header>

      <div className={rv.metrics}>
        {metrics.map((metric) => (
          <div key={metric.label} className={rv.metric}>
            <div className={rv.metricMain}>
              <span className={cx(rv.metricIcon, metric.tone)}>
                <Icon name={metric.icon} size={15} />
              </span>
              <span className={rv.metricText}>
                <span className={cx(rv.metricLabel, metric.tone)}>{metric.label}</span>
                <span className={rv.metricValue}>{metric.value}</span>
              </span>
            </div>
            <span className={cx(rv.metricTag, metric.tone)}>{metric.tag}</span>
          </div>
        ))}
      </div>

      <div role="tablist" aria-label={t('harness.review.tabsLabel')} className={rv.tabs}>
        {tabs.map((id) => {
          const count = groups.find((g) => g.id === id)?.findings.length;
          return (
            <button
              key={id}
              type="button"
              role="tab"
              aria-selected={tab === id}
              className={rv.tab}
              onClick={() => {
                setTab(id);
              }}
            >
              {id === 'business'
                ? t('harness.review.business')
                : t(`harness.review.groups.${id}` as TranslationKey)}
              {count !== undefined && <span className={rv.tabBadge}>{count}</span>}
            </button>
          );
        })}
      </div>

      <div className={rv.grid}>
        <div className={rv.column} role="tabpanel">
          <ConflictsPanel
            conflicts={analysis.conflicts}
            decisions={review.values}
            onChoose={(id, value) => {
              onChange({ ...review, values: { ...review.values, [id]: value } });
            }}
          />

          {current && (
            <section className={rv.card}>
              <h3 className={rv.cardHead}>
                <span className={rv.cardTitle}>
                  {t(`harness.review.groups.${current.id}` as TranslationKey)}
                  <span className={rv.cardCount}>{current.findings.length}</span>
                </span>
              </h3>
              {current.findings.map((finding) => (
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
                          onChange({
                            ...review,
                            values: { ...review.values, [finding.id]: value },
                          });
                        }
                      : undefined
                  }
                  selected={inspectedId === finding.id}
                  onInspect={() => {
                    setInspectedId(finding.id);
                  }}
                />
              ))}
            </section>
          )}
          {tab === tabs[0] && groups.every((g) => g.id !== 'architecture') && (
            <p className={rv.note}>{t('harness.architecture.unknown')}</p>
          )}

          {tab === 'business' && (
            <>
              <section className={rv.card}>
                <h3 className={rv.cardHead}>
                  <span className={rv.cardTitle}>{t('harness.review.business')}</span>
                </h3>
                <p className={rv.note}>{t('harness.review.optional')}</p>
                {USER_FIELDS.map(([field, label, kind]) => (
                  <label key={field} className={rv.field}>
                    <span className={rv.fieldLabel}>{t(label)}</span>
                    {review.suggested.includes(field) && (
                      <span className={rv.draft}>{t('harness.review.suggested')}</span>
                    )}
                    {kind === 'input' ? (
                      <input
                        className={rv.control}
                        value={review[field]}
                        onChange={(event) => {
                          onChange({ ...review, [field]: event.target.value });
                        }}
                      />
                    ) : (
                      <textarea
                        className={rv.control}
                        rows={3}
                        value={review[field]}
                        onChange={(event) => {
                          onChange({ ...review, [field]: event.target.value });
                        }}
                      />
                    )}
                  </label>
                ))}
                <p className={rv.note}>{t('harness.review.keepNote')}</p>
                <p className={rv.note}>{t('harness.review.secretsNote')}</p>
                {analysis.unmanagedUserFiles.length > 0 && (
                  <p role="status" className={cx(rv.note, rv.warnNote)}>
                    {t('harness.review.unmanaged', {
                      files: analysis.unmanagedUserFiles.join(', '),
                    })}
                  </p>
                )}
              </section>
            </>
          )}
        </div>

        <aside className={rv.column} aria-label={t('harness.review.inspector.title')}>
          <div className={rv.inspector}>
            <div className={rv.inspectorHead}>
              <h3 className={rv.cardTitle}>
                <Icon name="search" size={16} />
                {t('harness.review.inspector.title')}
              </h3>
              {inspected?.evidence[0] && (
                <span className={rv.inspectorSource} title={inspected.evidence[0].source}>
                  {shortPath(inspected.evidence[0].source)}
                </span>
              )}
            </div>
            {inspected ? (
              <div
                role="region"
                aria-label={t('harness.evidence.of', { label: findingName(t, inspected) })}
                className={rv.column}
              >
                <h4 className={rv.inspectorName}>{inspectorName(findingName(t, inspected))}</h4>
                <EvidenceDetails finding={inspected} />
              </div>
            ) : (
              <p className={rv.empty}>{t('harness.review.inspector.empty')}</p>
            )}
          </div>
        </aside>
      </div>

      {error && <ErrorBox problem={error} />}

      <footer className={rv.footer}>
        <span className={rv.footerState}>
          <span className={rv.dot} aria-hidden="true" />
          {t('harness.review.state')}
        </span>
        <label className={rv.switch}>
          <input
            type="checkbox"
            checked={review.ignoreInGit}
            onChange={(event) => {
              onChange({ ...review, ignoreInGit: event.target.checked });
            }}
          />
          <span>
            {t('harness.review.ignoreInGit')}
            <span className={rv.switchHelp}> {t('harness.review.ignoreInGitHelp')}</span>
          </span>
        </label>
      </footer>
    </form>
  );
}
