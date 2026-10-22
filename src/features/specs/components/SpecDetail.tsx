import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { PageHeader } from '@/components/ui/PageHeader';
import { useBreadcrumb } from '@/app/BreadcrumbContext';
import { OverflowMenu } from '@/components/ui/OverflowMenu';
import type { Agent } from '@/features/agents/types';
import type { Workspace } from '@/features/workspace/types';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import { useSpec } from '../hooks/useSpec';
import { dockFor, type DockActionId } from '../model/dock';
import { stageName } from '../model/messages';
import { statusView } from '../model/status';
import {
  answerFindings,
  raiseBudget,
  setSpecIntegrations,
  answerQuestion,
  approveSpec,
  archiveSpec,
  archiveWithCli,
  checkWithCli,
  assignSpecAgents,
  cancelRun,
  decideArtifact,
  deleteSpec,
  generateArtifact,
  getSpec,
  keepArtifact,
  publishSpec,
  resolveFinding,
  runReview,
  startSpec,
  validateSpec,
  waiveReview,
} from '../services/specService';
import {
  STAGES,
  type FindingStatus,
  type SpecArtifact,
  type SpecFinding,
  type SpecQuestion,
  type SpecView,
  type Stage,
} from '../types';
import { AgentsDialog } from './AgentsDialog';
import { DeleteSpecDialog } from './DeleteSpecDialog';
import { ArtifactDocument } from './ArtifactDocument';
import { OpenQuestions } from './OpenQuestions';
import { UsageBar } from './UsageBar';
import { AnswerQuestionsDialog } from './AnswerQuestionsDialog';
import { openQuestionsOf } from '../model/openQuestions';
import { Dock } from './Dock';
import { FinalReview } from './FinalReview';
import { Inspector } from './Inspector';
import { QuestionCard } from './QuestionCard';
import { RequestChangesDialog } from './RequestChangesDialog';
import { RunBanner } from './RunBanner';
import { StatusBadge } from './StatusBadge';
import { Stepper } from './Stepper';
import styles from './Specs.module.css';

interface Props {
  workspace: Workspace;
  agents: Agent[];
  specId: string;
  onBack: () => void;
  onImplement: (specId: string) => void;
  onDeleted: () => void;
}

type Dialog = 'revise' | 'redo' | 'answers' | 'agents' | 'delete' | null;

/** Where the person should be looking: the first document not approved, else the final review. */
export function defaultStage(view: SpecView): Stage {
  if (!view.spec.started) return 'proposal';
  const open = STAGES.find((stage) => stage !== 'final' && view.phases[stage] !== 'approved');
  return open ?? 'final';
}

export function SpecDetail({ workspace, agents, specId, onBack, onImplement, onDeleted }: Props) {
  const { t } = useI18n();
  const { state, busy, error, launchError, act, dismiss } = useSpec(specId);
  const [picked, setPicked] = useState<Stage | null>(null);
  const [dialog, setDialog] = useState<Dialog>(null);
  const [focus, setFocus] = useState<string | null>(null);

  const nameOf = (id: string) => agents.find((a) => a.id === id)?.name ?? '—';
  useBreadcrumb([state.status === 'ready' && state.view.spec.title]);

  if (state.status === 'loading') {
    return (
      <p className={styles.muted} role="status">
        {t('specs.loading')}
      </p>
    );
  }
  if (state.status === 'error') {
    return (
      <div className={styles.stack}>
        <p className={styles.error} role="alert">
          {errorMessage(t, state.error)}
        </p>
        <Button variant="secondary" onClick={onBack}>
          {t('specs.back')}
        </Button>
      </div>
    );
  }

  const { view } = state;
  const { spec } = view;
  const stage = picked ?? defaultStage(view);
  const artifact = stage === 'final' ? undefined : spec.artifacts.find((a) => a.id === stage);
  const status = statusView(view.status);
  const canArchive =
    view.spec.publication.state === 'published' && view.publicationCurrent && !view.spec.archived;
  const nextStage = STAGES[STAGES.indexOf(stage) + 1];
  const questions = spec.questions.filter(
    (q) => q.status === 'pending' && q.source === 'artifact' && q.artifactId === stage,
  );

  const go = (target: Stage) => {
    setPicked(target);
    setFocus(null);
  };

  const onDock = async (id: DockActionId) => {
    if (!artifact) return;
    const revision = artifact.revisions.at(-1)?.number ?? 0;
    switch (id) {
      case 'approve':
        if (await act(() => decideArtifact(specId, artifact.id, revision, 'approve', null))) {
          setPicked(null);
        }
        break;
      case 'revise':
      case 'redo':
        setDialog(id);
        break;
      case 'generate':
        await act(() => generateArtifact(specId, artifact.id));
        break;
      case 'cancel':
        await act(async () => {
          await cancelRun(specId);
          return getSpec(specId);
        });
        break;
      case 'keep':
        await act(() => keepArtifact(specId, artifact.id, revision));
        break;
    }
  };

  const sendDecision = async (kind: 'revise' | 'redo', note: string) => {
    if (!artifact) return;
    const revision = artifact.revisions.at(-1)?.number ?? 0;
    const sent = await act(() =>
      decideArtifact(specId, artifact.id, revision, kind, note === '' ? null : note),
    );
    if (sent) setDialog(null);
  };

  const answer = (question: SpecQuestion, choice: string | null, text: string | null) =>
    act(() => answerQuestion(specId, question.id, choice, text));

  const onAnswerFindings = async (answers: { findingId: string; answer: string }[]) => {
    // The first document, in the order they are written, is the one that is being rewritten now.
    const targets = answers
      .map((a) => spec.findings.find((f) => f.id === a.findingId)?.artifactId)
      .filter((id): id is string => Boolean(id));
    const first = STAGES.find((stage) => targets.includes(stage));
    const sent = await act(() => answerFindings(specId, answers));
    if (sent && first) go(first);
    return sent;
  };

  const onResolve = (findingId: string, resolved: FindingStatus, note: string | null) =>
    act(() => resolveFinding(specId, findingId, resolved, note));

  const showFinding = (finding: SpecFinding) => {
    if (!finding.artifactId) return;
    setFocus(finding.reference);
    setPicked(finding.artifactId as Stage);
  };

  const openOf = (current: SpecArtifact) =>
    current.revisions.at(-1)?.files.flatMap((f) => openQuestionsOf(f.content)) ?? [];

  const pane = (current: SpecArtifact) => {
    const phase = view.phases[current.id];
    return (
      <div className={styles.stack}>
        <RunBanner
          artifact={current}
          agentName={nameOf(spec.authorAgentId)}
          busy={busy}
          onCancel={() => void onDock('cancel')}
        />
        {questions.map((question) => (
          <QuestionCard
            key={question.id}
            question={question}
            busy={busy}
            onAnswer={(choice, text) => void answer(question, choice, text)}
          />
        ))}
        {current.revisions.length > 0 && (
          <OpenQuestions
            questions={openOf(current)}
            busy={busy}
            onAnswer={() => {
              setDialog('answers');
            }}
          />
        )}
        {current.revisions.length > 0 ? (
          <ArtifactDocument
            key={`${current.id}-${String(current.revisions.at(-1)?.number ?? 0)}`}
            view={view}
            artifact={current}
            focus={focus}
          />
        ) : (
          phase && (
            <div className={styles.placeholder} role="status">
              <Icon name="specs" size={22} />
              <h3>{stageName(t, current.id)}</h3>
              <p>
                {t(`specs.pane.${phase}`, {
                  stage: stageName(t, current.id),
                  needs: current.dependsOn.map((id) => stageName(t, id)).join(', '),
                })}
              </p>
            </div>
          )
        )}
      </div>
    );
  };

  return (
    <section className={styles.detail} aria-label={spec.title}>
      <PageHeader
        level="h2"
        back={{ label: t('specs.back'), onClick: onBack }}
        title={spec.title}
        titleAside={<StatusBadge tone={status.tone}>{t(status.key)}</StatusBadge>}
        meta={
          <span className={styles.meta}>
            <span>{spec.changeId}</span>
            <span>{t('specs.meta.format', { schema: spec.frameworkSchema })}</span>
            {workspace.projects.length > 1 && (
              <span>{workspace.projects.map((p) => p.name).join(', ')}</span>
            )}
            <span>
              {t('specs.meta.author')}: <b>{nameOf(spec.authorAgentId)}</b>
            </span>
            <span>
              {t('specs.meta.reviewer')}: <b>{nameOf(spec.reviewerAgentId)}</b>
            </span>
            {view.integrations.length > 0 && (
              <span>
                {t('specs.meta.integrations')}: {view.integrations.map((i) => i.name).join(', ')}
              </span>
            )}
            {spec.origin.kind === 'imported' && <span>{t('specs.meta.imported')}</span>}
          </span>
        }
        actions={
          <>
            <Button
              variant="secondary"
              disabled={busy || !!spec.archived}
              onClick={() => {
                setDialog('agents');
              }}
            >
              <Icon name="settings" size={14} />
              {t('specs.agents.action')}
            </Button>
            {!busy && !view.busy && (canArchive || spec.implementations.length === 0) && (
              <OverflowMenu
                label={t('specs.moreActions', { title: spec.title })}
                items={[
                  ...(canArchive
                    ? [
                        {
                          label: t('specs.archive'),
                          icon: 'stack' as const,
                          onSelect: () => {
                            void act(() => archiveSpec(specId));
                          },
                        },
                      ]
                    : []),
                  ...(spec.implementations.length === 0
                    ? [
                        {
                          label: t('common.delete'),
                          icon: 'trash' as const,
                          danger: true,
                          onSelect: () => {
                            setDialog('delete');
                          },
                        },
                      ]
                    : []),
                ]}
              />
            )}
          </>
        }
      />

      {spec.started && <Stepper view={view} selected={stage} onSelect={go} />}

      <UsageBar
        part="banner"
        view={view}
        busy={busy}
        onContinue={() => void act(() => raiseBudget(specId))}
      />

      {error !== null && (
        <div className={styles.banner} data-tone="danger" role="alert">
          <Icon name="warning" size={16} />
          <div className={styles.bannerText}>
            <strong>{errorMessage(t, error)}</strong>
          </div>
          <Button variant="ghost" onClick={dismiss}>
            {t('common.close')}
          </Button>
        </div>
      )}
      {launchError !== null && (
        <div className={styles.banner} data-tone="warning" role="alert">
          <Icon name="warning" size={16} />
          <div className={styles.bannerText}>
            <strong>{t('specs.launchFailed')}</strong>
            <span>{errorMessage(t, launchError)}</span>
          </div>
          <Button variant="ghost" onClick={dismiss}>
            {t('common.close')}
          </Button>
        </div>
      )}

      {!spec.started ? (
        <div className={styles.startPanel}>
          <h3>{t('specs.start.title')}</h3>
          <p className={styles.requestText}>{spec.request}</p>
          {spec.context && (
            <>
              <h4>{t('specs.new.context')}</h4>
              <p className={styles.requestText}>{spec.context}</p>
            </>
          )}
          <p className={styles.muted}>
            {t('specs.start.explain', { agent: nameOf(spec.authorAgentId) })}
          </p>
          <div className={styles.row}>
            <Button
              disabled={busy || !view.authorAvailable || !view.projectAvailable}
              onClick={() => void act(() => startSpec(specId))}
            >
              {t('specs.start.action')}
            </Button>
          </div>
          {!view.authorAvailable && <p className={styles.error}>{t('specs.why.author_missing')}</p>}
        </div>
      ) : stage === 'final' ? (
        <div className={styles.workArea} data-full="true">
          <div className={styles.main}>
            <FinalReview
              view={view}
              busy={busy}
              reviewerName={nameOf(spec.reviewerAgentId)}
              onGoto={go}
              onRevalidate={() => void act(() => validateSpec(specId))}
              onRunReview={() => void act(() => runReview(specId))}
              onWaive={(note) => void act(() => waiveReview(specId, note))}
              onAnswerQuestion={(question, choice, text) => void answer(question, choice, text)}
              onAnswerFindings={onAnswerFindings}
              onResolve={(id, resolved, note) => void onResolve(id, resolved, note)}
              onShowFinding={showFinding}
              onApprove={() => void act(() => approveSpec(specId))}
              onPublish={(changeId) => void act(() => publishSpec(specId, changeId))}
              onArchive={() => void act(() => archiveSpec(specId))}
              onImplement={() => {
                onImplement(specId);
              }}
              error={error}
              onCliCheck={() => void act(() => checkWithCli(specId))}
              onCliArchive={() => void act(() => archiveWithCli(specId))}
              onCancel={() =>
                void act(async () => {
                  await cancelRun(specId);
                  return getSpec(specId);
                })
              }
            />
            <UsageBar
              part="panel"
              view={view}
              busy={busy}
              onContinue={() => void act(() => raiseBudget(specId))}
            />
          </div>
        </div>
      ) : (
        artifact && (
          <>
            <div className={styles.workArea}>
              <div className={styles.main}>{pane(artifact)}</div>
              <aside className={styles.inspector} aria-label={t('specs.inspector.title')}>
                <Inspector
                  view={view}
                  artifact={artifact}
                  busy={busy}
                  onRevalidate={() => void act(() => validateSpec(specId))}
                  onFindings={() => {
                    go('final');
                  }}
                />
                <UsageBar
                  part="panel"
                  view={view}
                  busy={busy}
                  onContinue={() => void act(() => raiseBudget(specId))}
                />
              </aside>
            </div>
            <Dock
              dock={dockFor(view, artifact.id)}
              busy={busy}
              stage={stageName(t, artifact.id)}
              next={nextStage ? stageName(t, nextStage) : ''}
              onAction={(id) => void onDock(id)}
            />
          </>
        )
      )}

      {(dialog === 'revise' || dialog === 'redo') && artifact && (
        <RequestChangesDialog
          mode={dialog}
          busy={busy}
          targets={artifact.revisions.at(-1)?.files.map((f) => f.path) ?? []}
          onClose={() => {
            setDialog(null);
          }}
          onSend={(note) => void sendDecision(dialog, note)}
        />
      )}
      {dialog === 'answers' && artifact && (
        <AnswerQuestionsDialog
          questions={openOf(artifact)}
          targets={artifact.revisions.at(-1)?.files.map((f) => f.path) ?? []}
          busy={busy}
          onClose={() => {
            setDialog(null);
          }}
          onSend={(note) => void sendDecision('revise', note)}
        />
      )}
      {dialog === 'agents' && (
        <AgentsDialog
          workspace={workspace}
          agents={agents}
          view={view}
          busy={busy}
          onClose={() => {
            setDialog(null);
          }}
          onSave={(author, reviewer, integrations) => {
            void act(async () => {
              await assignSpecAgents(specId, author, reviewer);
              return setSpecIntegrations(specId, integrations);
            }).then((saved) => {
              if (saved) setDialog(null);
            });
          }}
        />
      )}
      {dialog === 'delete' && (
        <DeleteSpecDialog
          title={spec.title}
          busy={busy}
          onClose={() => {
            setDialog(null);
          }}
          onConfirm={() => {
            void act(async () => {
              await deleteSpec(specId);
              onDeleted();
              return view;
            });
          }}
        />
      )}
    </section>
  );
}
