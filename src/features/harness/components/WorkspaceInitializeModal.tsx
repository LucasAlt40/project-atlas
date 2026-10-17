import { useEffect, useState, type ReactNode } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import {
  analyzeProject,
  buildWorkspaceContext,
  initializeProject,
} from '@/features/workspace/services/workspaceService';
import { useT } from '@/i18n/I18nProvider';
import { errorDetail, errorMessage } from '@/i18n/messages';
import type {
  InitModeDto,
  InitializeOutcomeDto,
  ProjectAnalysisDto,
  SemanticRequestDto,
} from '@/lib/tauri/commands';
import { buildInput, initialReview, type ReviewState } from '../model/review';
import { AgentAnalysisScreen } from './AgentAnalysisScreen';
import { ErrorBox, type Problem } from './ErrorBox';
import { HarnessFrame, type HarnessPresentation } from './HarnessFrame';
import { HealthBadge } from './HealthBadge';
import { FindingsStep, ReviewStep, useSemanticForm } from './InitializeProjectModal';
import styles from './Harness.module.css';
import rv from './Review.module.css';

export interface ProjectRef {
  id: string;
  name: string;
  path: string;
}

interface Props {
  workspaceId: string;
  /** The projects analysed together. */
  projects: ProjectRef[];
  onClose: () => void;
  /** Called once Harnesses were written, so the screen behind can read them again. */
  onDone: () => void;
  /** Over the screen (default), or in the page, with the steps of the flow on top. */
  presentation?: HarnessPresentation;
}

interface Item {
  status: 'analyzing' | 'ready' | 'failed';
  analysis?: ProjectAnalysisDto;
  problem?: Problem;
  /** What went wrong asking the model about this project; the earlier analysis stays. */
  aiProblem?: Problem | undefined;
  review?: ReviewState;
  mode?: InitModeDto;
  outcome?: InitializeOutcomeDto;
  submitProblem?: Problem | undefined;
}

type Stage = 'findings' | 'review' | 'done';

function problemOf(t: ReturnType<typeof useT>, error: unknown): Problem {
  return { message: errorMessage(t, error), detail: errorDetail(error) };
}

/**
 * Initialize Projects: every project of the workspace is analysed at once, reviewed in the same
 * full review as a single project (one tab each) and initialized together. One AI analysis, with
 * one agent and one note, covers them all. Nothing is written until the user confirms, a model
 * only ever proposes, and one project failing never holds the others back.
 */
export function WorkspaceInitializeModal({
  workspaceId,
  projects,
  onClose,
  onDone,
  presentation = 'modal',
}: Props) {
  const t = useT();
  const ai = useSemanticForm();
  const [items, setItems] = useState<Record<string, Item>>(() =>
    Object.fromEntries(projects.map((p) => [p.id, { status: 'analyzing' } satisfies Item])),
  );
  const [stage, setStage] = useState<Stage>('findings');
  const [activeId, setActiveId] = useState(projects[0]?.id ?? '');
  const [busy, setBusy] = useState(false);
  const [running, setRunning] = useState<{ agentId: string; agentName: string } | null>(null);

  const patch = (id: string, change: Partial<Item>) => {
    setItems((current) => ({ ...current, [id]: { ...current[id], ...change } as Item }));
  };

  // Every project is analysed at the same time: the core serves them in parallel.
  useEffect(() => {
    let cancelled = false;
    for (const project of projects) {
      analyzeProject(workspaceId, undefined, project.id)
        .then((analysis) => {
          if (!cancelled) patch(project.id, { status: 'ready', analysis });
        })
        .catch((e: unknown) => {
          if (!cancelled) patch(project.id, { status: 'failed', problem: problemOf(t, e) });
        });
    }
    return () => {
      cancelled = true;
    };
    // The projects of a run are fixed when it starts.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [workspaceId]);

  const item = (id: string): Item => items[id] ?? { status: 'analyzing' };
  const loading = projects.some((p) => item(p.id).status === 'analyzing');
  const ready = projects.filter((p) => item(p.id).status === 'ready');
  const active = item(activeId);
  const names = Object.fromEntries(projects.map((p) => [p.path, p.name]));
  const aiFailed = ready.filter(
    (p) =>
      item(p.id).aiProblem !== undefined ||
      item(p.id).analysis?.analysis.semantic.status === 'failed',
  );
  const aiWarning = (
    <>
      {aiFailed.length > 0 && (
        <p role="alert" className={styles.warning}>
          {t('harness.multi.aiFailed', { names: aiFailed.map((p) => p.name).join(', ') })}
        </p>
      )}
    </>
  );

  function runSemantic() {
    const request: SemanticRequestDto = {
      agentId: ai.chosen,
      instructions: ai.instructions.trim(),
      restricted: ai.restricted,
    };
    const targets = ready.filter(
      (p) => item(p.id).analysis?.analysis.semantic.status !== 'completed',
    );
    setBusy(true);
    setRunning({
      agentId: request.agentId,
      agentName: ai.agents.find((a) => a.id === request.agentId)?.name ?? request.agentId,
    });
    void Promise.all(
      targets.map((project) =>
        analyzeProject(workspaceId, request, project.id)
          .then((analysis) => {
            patch(project.id, { analysis, aiProblem: undefined });
          })
          .catch((e: unknown) => {
            patch(project.id, { aiProblem: problemOf(t, e) });
          }),
      ),
    ).finally(() => {
      setBusy(false);
      setRunning(null);
    });
  }

  function reviewAll() {
    for (const project of ready) {
      const { analysis, review } = item(project.id);
      if (!analysis) continue;
      const existing = analysis.existing?.status;
      patch(project.id, {
        review: review ?? initialReview(analysis),
        mode:
          existing === 'initialized' || existing === 'needs_review' ? 'update_existing' : 'create',
      });
    }
    setStage('review');
  }

  function submitAll() {
    const targets = ready.filter((p) => !item(p.id).outcome);
    setBusy(true);
    void Promise.all(
      targets.map((project) => {
        const { analysis, review, mode } = item(project.id);
        if (!analysis || !review || !mode) return Promise.resolve();
        return initializeProject(workspaceId, buildInput(analysis, review, mode), project.id)
          .then((outcome) => {
            patch(project.id, { outcome, submitProblem: undefined });
          })
          .catch((e: unknown) => {
            patch(project.id, { submitProblem: problemOf(t, e) });
          });
      }),
    )
      .then(() => buildWorkspaceContext(workspaceId))
      .catch(() => undefined)
      .finally(() => {
        setBusy(false);
        setStage('done');
        onDone();
      });
  }

  const tabs = (
    <div role="tablist" aria-label={t('harness.multi.tabs')} className={rv.tabs}>
      {projects.map((project) => {
        const state = item(project.id);
        const mark =
          state.outcome !== undefined
            ? '✓'
            : state.status === 'analyzing'
              ? '…'
              : state.status === 'failed' ||
                  state.submitProblem ||
                  state.aiProblem ||
                  state.analysis?.analysis.semantic.status === 'failed'
                ? '!'
                : '';
        return (
          <button
            key={project.id}
            type="button"
            role="tab"
            aria-selected={activeId === project.id}
            className={rv.tab}
            onClick={() => {
              setActiveId(project.id);
            }}
          >
            {project.name}
            {mark && <span aria-hidden="true"> {mark}</span>}
          </button>
        );
      })}
    </div>
  );

  const failedProject = (id: string): ReactNode => {
    const problem = item(id).problem;
    return problem ? <ErrorBox problem={problem} /> : null;
  };

  const done = projects.filter((p) => item(p.id).outcome !== undefined);
  const failedAtWriting = projects.filter((p) => item(p.id).submitProblem !== undefined);

  return (
    <>
      {running && (
        <AgentAnalysisScreen
          workspaceId={workspaceId}
          agentId={running.agentId}
          agentName={running.agentName}
          projects={names}
        />
      )}
      <HarnessFrame
        label={t('harness.multi.title')}
        onClose={onClose}
        presentation={presentation}
        size={stage === 'review' ? 'wide' : stage === 'findings' ? 'analyze' : 'normal'}
        step={stage === 'review' ? 2 : stage === 'done' ? 3 : 1}
        finished={stage === 'done'}
      >
        <div className={styles.modal}>
          {stage === 'findings' && (
            <>
              {loading && (
                <p role="status" className={styles.muted}>
                  {t('harness.multi.analyzing', {
                    count: String(projects.filter((p) => item(p.id).status === 'analyzing').length),
                  })}
                </p>
              )}
              {active.status === 'ready' && active.analysis ? (
                <FindingsStep
                  key={activeId}
                  analysis={active.analysis}
                  busy={busy}
                  error={active.aiProblem ?? null}
                  form={ai}
                  header={
                    <>
                      {tabs}
                      {aiWarning}
                    </>
                  }
                  onCancel={onClose}
                  onReview={reviewAll}
                  onUseExisting={reviewAll}
                  onSemantic={runSemantic}
                  footer={
                    <footer className={[rv.footer, rv.analyzeFooter].join(' ')}>
                      <button type="button" className={rv.textButton} onClick={onClose}>
                        {t('common.cancel')}
                      </button>
                      <div className={rv.footerActions}>
                        <button
                          type="button"
                          className={rv.ghostButton}
                          disabled={busy || loading || ready.length === 0}
                          onClick={reviewAll}
                        >
                          <Icon name="eye" size={15} />
                          {t('harness.multi.reviewAll', { count: String(ready.length) })}
                        </button>
                        {ai.canRun && (
                          <button
                            type="button"
                            className={rv.primaryButton}
                            disabled={busy || loading || !ai.chosen || ready.length === 0}
                            onClick={runSemantic}
                          >
                            <Icon name="sparkle" size={15} />
                            {busy ? t('harness.semantic.running') : t('harness.multi.runAll')}
                          </button>
                        )}
                      </div>
                    </footer>
                  }
                />
              ) : (
                <>
                  {tabs}
                  {active.status === 'analyzing' ? (
                    <p className={styles.muted}>{t('harness.analyzing')}</p>
                  ) : (
                    failedProject(activeId)
                  )}
                  <div className={styles.actions}>
                    <Button variant="secondary" onClick={onClose}>
                      {t('common.cancel')}
                    </Button>
                  </div>
                </>
              )}
            </>
          )}

          {stage === 'review' &&
            (active.status === 'ready' && active.analysis && active.review && active.mode ? (
              <ReviewStep
                key={activeId}
                analysis={active.analysis}
                review={active.review}
                busy={busy}
                error={active.submitProblem ?? null}
                header={
                  <>
                    {tabs}
                    {aiWarning}
                  </>
                }
                submitLabel={t('harness.multi.initializeAll', { count: String(ready.length) })}
                onChange={(review) => {
                  patch(activeId, { review });
                }}
                onBack={() => {
                  setStage('findings');
                }}
                onSubmit={submitAll}
              />
            ) : (
              <>
                {tabs}
                {failedProject(activeId)}
                <div className={styles.actions}>
                  <Button
                    variant="secondary"
                    onClick={() => {
                      setStage('findings');
                    }}
                  >
                    {t('harness.back')}
                  </Button>
                </div>
              </>
            ))}

          {stage === 'done' && (
            <>
              <h2 className={styles.title}>{t('harness.done.title')}</h2>
              <p role="status">
                {t('harness.multi.done', {
                  done: String(done.length),
                  total: String(projects.length),
                })}
              </p>
              <ul className={styles.list}>
                {projects.map((project) => {
                  const state = item(project.id);
                  return (
                    <li key={project.id}>
                      {state.outcome ? (
                        <>
                          <span className={styles.ok}>✓</span> <strong>{project.name}</strong>{' '}
                          {state.outcome.summary.health && (
                            <HealthBadge health={state.outcome.summary.health} />
                          )}
                        </>
                      ) : (
                        <>
                          <strong>{project.name}</strong>
                          {(state.submitProblem ?? state.problem) && (
                            <ErrorBox
                              problem={state.submitProblem ?? state.problem ?? { message: '' }}
                            />
                          )}
                        </>
                      )}
                    </li>
                  );
                })}
              </ul>
              {done.length > 0 && <p>{t('harness.agentsCanUse')}</p>}
              <div className={styles.actions}>
                {failedAtWriting.length > 0 && (
                  <Button disabled={busy} onClick={submitAll}>
                    {t('harness.multi.retry')}
                  </Button>
                )}
                <Button variant="secondary" onClick={onClose}>
                  {t('common.close')}
                </Button>
              </div>
            </>
          )}
        </div>
      </HarnessFrame>
    </>
  );
}
