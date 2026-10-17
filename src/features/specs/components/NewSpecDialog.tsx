import { useEffect, useMemo, useState, type SyntheticEvent } from 'react';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import type { Agent } from '@/features/agents/types';
import type { Workspace } from '@/features/workspace/types';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import { listWorkspaceProjects } from '@/features/workspace/services/workspaceService';
import type { ProjectInfoDto } from '@/lib/tauri/commands';
import { agentsOf, authorName, isRecommendedAuthor, suggestedAuthor } from '../model/agents';
import { IntegrationsPicker } from './IntegrationsPicker';
import { contextFromAnswers, profileOf, questionsFor } from '../model/contextQuestions';
import { createSpec } from '../services/specService';
import type { SpecView } from '../types';
import styles from './Specs.module.css';

const TITLE_MAX = 120;
const REQUEST_MAX = 8_000;

interface Props {
  workspace: Workspace;
  agents: Agent[];
  onClose: () => void;
  onCreated: (view: SpecView) => void;
}

/**
 * The request for a new spec. Creating it records the request and runs nothing: the person
 * starts it from the spec, once they have looked at it.
 */
export function NewSpecDialog({ workspace, agents, onClose, onCreated }: Props) {
  const { t } = useI18n();
  const inWorkspace = useMemo(() => agentsOf(workspace, agents), [workspace, agents]);
  const [title, setTitle] = useState('');
  const [request, setRequest] = useState('');
  const [context, setContext] = useState('');
  const [projectId, setProjectId] = useState(workspace.projects[0]?.id ?? '');
  const [author, setAuthor] = useState(suggestedAuthor(inWorkspace)?.id ?? '');
  const [reviewer, setReviewer] = useState('');
  const [projects, setProjects] = useState<ProjectInfoDto[]>([]);
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [integrations, setIntegrations] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>(null);

  useEffect(() => {
    let current = true;
    listWorkspaceProjects(workspace.id)
      .then((found) => {
        if (current) setProjects(found);
      })
      .catch(() => undefined);
    return () => {
      current = false;
    };
  }, [workspace.id]);

  const project = projects.find((p) => p.id === projectId);
  const profile = useMemo(
    () => profileOf(project?.role ?? null, project?.technologies ?? []),
    [project],
  );
  const asked = useMemo(
    () => (request.trim().length >= 12 ? questionsFor(request, profile) : []),
    [request, profile],
  );
  const kind =
    profile.ui && profile.server && profile.data
      ? 'all'
      : profile.data
        ? 'data'
        : profile.server
          ? 'server'
          : 'ui';

  const valid = title.trim() !== '' && request.trim() !== '' && author !== '';

  async function submit(event: SyntheticEvent) {
    event.preventDefault();
    if (!valid || busy) return;
    setBusy(true);
    setError(null);
    try {
      const view = await createSpec({
        workspaceId: workspace.id,
        projectId,
        title: title.trim(),
        request: request.trim(),
        context: contextFromAnswers(
          context,
          asked.map((q) => ({ question: t(`specs.ctxq.${q.id}`), answer: answers[q.id] ?? '' })),
        ),
        authorAgentId: author,
        reviewerAgentId: reviewer || author,
        integrations,
      });
      onCreated(view);
    } catch (failure: unknown) {
      setError(failure);
      setBusy(false);
    }
  }

  return (
    <Modal label={t('specs.new')} onClose={onClose} size="wide">
      <form className={styles.form} onSubmit={(event) => void submit(event)}>
        <h2 className={styles.dialogTitle}>{t('specs.new')}</h2>
        <p className={styles.muted}>{t('specs.new.intro')}</p>
        <div className={styles.formBody}>
          <div className={styles.formColumns}>
            <div className={styles.formCol}>
              <label className={styles.field}>
                <span>{t('specs.new.title')}</span>
                <input
                  value={title}
                  maxLength={TITLE_MAX}
                  placeholder={t('specs.new.titlePlaceholder')}
                  onChange={(event) => {
                    setTitle(event.target.value);
                  }}
                />
              </label>
              {workspace.projects.length > 1 && (
                <label className={styles.field}>
                  <span>{t('specs.new.project')}</span>
                  <select
                    value={projectId}
                    onChange={(event) => {
                      setProjectId(event.target.value);
                    }}
                  >
                    {workspace.projects.map((project) => (
                      <option key={project.id} value={project.id}>
                        {project.name}
                      </option>
                    ))}
                  </select>
                </label>
              )}
              <label className={styles.field}>
                <span>{t('specs.new.request')}</span>
                <textarea
                  rows={4}
                  value={request}
                  maxLength={REQUEST_MAX}
                  placeholder={t('specs.new.requestPlaceholder')}
                  onChange={(event) => {
                    setRequest(event.target.value);
                  }}
                />
                <small className={styles.hint}>{t('specs.new.requestHelp')}</small>
              </label>
              <label className={styles.field}>
                <span>{t('specs.new.ctx.extra')}</span>
                <textarea
                  rows={2}
                  value={context}
                  maxLength={REQUEST_MAX}
                  onChange={(event) => {
                    setContext(event.target.value);
                  }}
                />
                <small className={styles.hint}>{t('specs.new.ctx.extraHelp')}</small>
              </label>
            </div>
            <div className={styles.formCol}>
              <section className={styles.contextAsk} aria-label={t('specs.new.ctx.title')}>
                <h3>{t('specs.new.ctx.title')}</h3>
                <p className={styles.muted}>{t('specs.new.ctx.intro')}</p>
                {asked.length === 0 ? (
                  <p className={styles.hint}>{t('specs.new.ctx.waiting')}</p>
                ) : (
                  <>
                    <p className={styles.hint}>
                      {t('specs.new.ctx.profile', { kind: t(`specs.new.ctx.kind.${kind}`) })}
                    </p>
                    {asked.map((question) => (
                      <label key={question.id} className={styles.field}>
                        <span className={styles.questionText}>
                          {t(`specs.ctxq.${question.id}`)}
                        </span>
                        <small className={styles.hint}>
                          {t(`specs.ctxq.because.${question.because}`)}
                        </small>
                        <textarea
                          rows={2}
                          value={answers[question.id] ?? ''}
                          maxLength={1500}
                          placeholder={t('specs.new.ctx.placeholder')}
                          onChange={(event) => {
                            setAnswers((current) => ({
                              ...current,
                              [question.id]: event.target.value,
                            }));
                          }}
                        />
                      </label>
                    ))}
                  </>
                )}
              </section>
              {inWorkspace.length === 0 ? (
                <p className={styles.error} role="alert">
                  {t('specs.new.noAgents')}
                </p>
              ) : (
                <>
                  <div className={styles.fieldRow}>
                    <label className={styles.field}>
                      <span>{t('specs.new.author')}</span>
                      <select
                        value={author}
                        onChange={(event) => {
                          setAuthor(event.target.value);
                        }}
                      >
                        {inWorkspace.map((agent) => (
                          <option key={agent.id} value={agent.id}>
                            {authorName(t, agent)}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label className={styles.field}>
                      <span>{t('specs.new.reviewer')}</span>
                      <select
                        value={reviewer}
                        onChange={(event) => {
                          setReviewer(event.target.value);
                        }}
                      >
                        <option value="">{t('specs.new.sameAgent')}</option>
                        {inWorkspace.map((agent) => (
                          <option key={agent.id} value={agent.id}>
                            {agent.name}
                          </option>
                        ))}
                      </select>
                    </label>
                  </div>
                  {!inWorkspace.some(isRecommendedAuthor) && (
                    <small className={styles.hint}>{t('specs.new.authorHint')}</small>
                  )}
                </>
              )}
              <IntegrationsPicker
                workspaceId={workspace.id}
                selected={integrations}
                onChange={setIntegrations}
                preselect
              />
            </div>
          </div>
          <p className={styles.note}>{t('specs.new.nothingRuns')}</p>
          {error !== null && (
            <p className={styles.error} role="alert">
              {errorMessage(t, error)}
            </p>
          )}
        </div>
        <div className={styles.dialogActions}>
          <Button variant="secondary" onClick={onClose}>
            {t('common.cancel')}
          </Button>
          <Button type="submit" disabled={!valid || busy}>
            {t('specs.new.create')}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
