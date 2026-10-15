import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { Modal } from '@/components/ui/Modal';
import { InitializeProjectModal } from '@/features/harness/components/InitializeProjectModal';
import { HarnessStats } from '@/features/harness/components/HarnessStats';
import { HealthBadge } from '@/features/harness/components/HealthBadge';
import { RefreshHarnessModal } from '@/features/harness/components/RefreshHarnessModal';
import { StaleNotice } from '@/features/harness/components/StaleNotice';
import {
  WorkspaceInitializeModal,
  type ProjectRef,
} from '@/features/harness/components/WorkspaceInitializeModal';
import { useT } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import { pickFolder as nativePickFolder } from '@/lib/tauri/dialog';
import type {
  HarnessSummaryDto,
  ProjectHarnessInfoDto,
  ProjectHarnessStateDto,
  ProjectInfoDto,
} from '@/lib/tauri/commands';
import { useProjectsOverview } from '../hooks/useProjectsOverview';
import {
  addWorkspaceProject,
  buildWorkspaceContext,
  getWorkspaceNotes,
  refreshProjectHarness,
  resetProjectHarness,
  resetWorkspaceHarness,
  removeWorkspaceProject,
  saveWorkspaceNotes,
  updateWorkspaceProject,
} from '../services/workspaceService';
import type { Workspace } from '../types';
import { ProjectForm, type ProjectFormValues } from './ProjectForm';
import projectStyles from './Projects.module.css';
import styles from './Workspace.module.css';

interface Props {
  workspace: Workspace;
  /** The workspace as the core returned it after a project was added, changed or removed. */
  onWorkspaceChanged: (workspace: Workspace) => void;
  onClose: () => void;
  pickFolder?: (defaultPath?: string) => Promise<string | null>;
}

type Editing = { projectId: string } | null;
type Reviewing = { kind: 'initialize' | 'refresh'; projectId: string } | null;

/** The signature of what is registered: it changes when a project is added, removed or moved. */
function signatureOf(workspace: Workspace): string {
  return workspace.projects
    .map((p) => `${p.id}|${p.path}|${p.name}|${p.role ?? ''}|${p.dependsOn.join(',')}`)
    .join(';');
}

/**
 * The projects of a workspace: add or remove folders, see where each project's Harness stands,
 * start the Harness of the ones that have none and keep the context that relates them. Every
 * project keeps its own Harness in its own folder; removing one here never touches its files.
 */
export function WorkspaceProjectsPanel({
  workspace,
  onWorkspaceChanged,
  onClose,
  pickFolder = nativePickFolder,
}: Props) {
  const t = useT();
  const { state, reload } = useProjectsOverview(workspace.id, signatureOf(workspace));
  const [editing, setEditing] = useState<Editing>(null);
  const [reviewing, setReviewing] = useState<Reviewing>(null);
  const [confirmingRemoval, setConfirmingRemoval] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  /** The projects being analysed and initialized together, when that run is open. */
  const [together, setTogether] = useState<ProjectRef[] | null>(null);
  const [starting, setStarting] = useState(false);
  const [fixingId, setFixingId] = useState<string | null>(null);
  /** A project id, or `'all'`, waiting for the user to confirm wiping its Harness. */
  const [confirmingReset, setConfirmingReset] = useState<string | null>(null);

  async function addFolder() {
    setNotice(null);
    try {
      const chosen = await pickFolder();
      if (!chosen) return;
      onWorkspaceChanged(await addWorkspaceProject(workspace.id, { path: chosen }));
    } catch (e) {
      setNotice(errorMessage(t, e));
    }
  }

  async function save(values: ProjectFormValues) {
    if (!editing) return;
    const updated = await updateWorkspaceProject(workspace.id, editing.projectId, values);
    onWorkspaceChanged(updated);
    setEditing(null);
  }

  function remove(projectId: string) {
    setNotice(null);
    removeWorkspaceProject(workspace.id, projectId)
      .then(onWorkspaceChanged)
      .catch((e: unknown) => {
        setNotice(errorMessage(t, e));
      })
      .finally(() => {
        setConfirmingRemoval(null);
      });
  }

  /** Brings a project's Harness up to date in one click; opens the review when it cannot. */
  function fix(projectId: string) {
    setFixingId(projectId);
    setNotice(null);
    refreshProjectHarness(workspace.id, true, projectId)
      .then((outcome) => {
        if (!outcome.applied) setReviewing({ kind: 'refresh', projectId });
      })
      .catch((e: unknown) => {
        setNotice(errorMessage(t, e));
      })
      .finally(() => {
        setFixingId(null);
        reload();
      });
  }

  function reset(projectId: string) {
    setStarting(true);
    setNotice(null);
    (projectId === 'all'
      ? resetWorkspaceHarness(workspace.id)
      : resetProjectHarness(workspace.id, projectId)
    )
      .then(() => {
        setNotice(t('projects.resetDone'));
      })
      .catch((e: unknown) => {
        setNotice(errorMessage(t, e));
      })
      .finally(() => {
        setStarting(false);
        setConfirmingReset(null);
        reload();
      });
  }

  const ready = state.status === 'ready' ? state : null;
  const harnessOf = (id: string): ProjectHarnessInfoDto | undefined =>
    ready?.overview.projects.find((p) => p.projectId === id);
  const pending = ready
    ? ready.projects.filter((p) => harnessOf(p.id)?.state === 'not_initialized')
    : [];
  const nameOf = (id: string) => workspace.projects.find((p) => p.id === id)?.name ?? id;

  return (
    <Modal label={t('projects.title', { name: workspace.name })} onClose={onClose} size="wide">
      <div className={projectStyles.panel}>
        <h2 className={projectStyles.title}>{t('projects.title', { name: workspace.name })}</h2>
        <p className={projectStyles.intro}>{t('projects.intro')}</p>
        {notice && (
          <p role="alert" className={styles.notice}>
            {notice}
          </p>
        )}
        <div className={projectStyles.toolbar}>
          <Button variant="secondary" onClick={() => void addFolder()}>
            <Icon name="plus" size={14} />
            {t('projects.add')}
          </Button>
        </div>

        {state.status === 'loading' && <p className={projectStyles.muted}>…</p>}
        {state.status === 'error' && (
          <p role="alert" className={styles.notice}>
            {t('projects.loadFailed')}
          </p>
        )}
        {ready && (
          <ul className={projectStyles.list} aria-label={t('projects.list')}>
            {ready.projects.map((info) => {
              const harness = harnessOf(info.id);
              const stateOf: ProjectHarnessStateDto = harness?.state ?? 'not_initialized';
              return (
                <li key={info.id} className={projectStyles.row}>
                  {editing?.projectId === info.id ? (
                    <ProjectForm
                      initial={info}
                      others={workspace.projects.filter((p) => p.id !== info.id)}
                      onSubmit={save}
                      onCancel={() => {
                        setEditing(null);
                      }}
                      pickFolder={pickFolder}
                    />
                  ) : (
                    <ProjectRow
                      info={info}
                      state={stateOf}
                      summary={harness?.summary}
                      fixing={fixingId === info.id}
                      onFix={() => {
                        fix(info.id);
                      }}
                      names={nameOf}
                      confirming={confirmingRemoval === info.id}
                      canRemove={ready.projects.length > 1}
                      canReset={
                        info.exists &&
                        (harness?.state === 'ready' ||
                          harness?.state === 'outdated' ||
                          harness?.state === 'needs_review')
                      }
                      confirmingReset={confirmingReset === info.id}
                      onAskReset={() => {
                        setConfirmingReset(info.id);
                      }}
                      onCancelReset={() => {
                        setConfirmingReset(null);
                      }}
                      onReset={() => {
                        reset(info.id);
                      }}
                      onEdit={() => {
                        setEditing({ projectId: info.id });
                      }}
                      onReview={() => {
                        setReviewing({
                          kind:
                            harness?.state === 'ready' || harness?.state === 'outdated'
                              ? 'refresh'
                              : 'initialize',
                          projectId: info.id,
                        });
                      }}
                      onUpdate={() => {
                        setReviewing({ kind: 'initialize', projectId: info.id });
                      }}
                      onAskRemove={() => {
                        setConfirmingRemoval(info.id);
                      }}
                      onCancelRemove={() => {
                        setConfirmingRemoval(null);
                      }}
                      onRemove={() => {
                        remove(info.id);
                      }}
                    />
                  )}
                </li>
              );
            })}
          </ul>
        )}

        {ready && (
          <section className={projectStyles.section} aria-label={t('harness.title')}>
            <h3 className={projectStyles.sectionTitle}>{t('harness.title')}</h3>
            <p className={projectStyles.muted}>{t('projects.init.note')}</p>
            {pending.length > 0 ? (
              <div className={projectStyles.toolbar}>
                <Button
                  disabled={starting}
                  onClick={() => {
                    setTogether(pending.map(({ id, name, path }) => ({ id, name, path })));
                  }}
                >
                  {t('projects.init.all', { count: pending.length })}
                </Button>
              </div>
            ) : (
              <p className={projectStyles.muted}>{t('projects.init.none')}</p>
            )}
            {pending.length < ready.projects.length &&
              (confirmingReset === 'all' ? (
                <div role="group" aria-label={t('projects.resetAll')}>
                  <p className={projectStyles.problem}>{t('projects.resetAllConfirm')}</p>
                  <div className={projectStyles.toolbar}>
                    <button
                      type="button"
                      className={styles.danger}
                      disabled={starting}
                      onClick={() => {
                        reset('all');
                      }}
                    >
                      {t('projects.confirmReset')}
                    </button>
                    <button
                      type="button"
                      className={styles.linkButton}
                      onClick={() => {
                        setConfirmingReset(null);
                      }}
                    >
                      {t('common.cancel')}
                    </button>
                  </div>
                </div>
              ) : (
                <div className={projectStyles.toolbar}>
                  <Button
                    variant="secondary"
                    disabled={starting}
                    onClick={() => {
                      setConfirmingReset('all');
                    }}
                  >
                    {t('projects.resetAll')}
                  </Button>
                </div>
              ))}
          </section>
        )}

        {ready && (
          <WorkspaceContextSection
            workspace={workspace}
            info={ready.overview.context}
            onRebuilt={reload}
          />
        )}
      </div>
      {reviewing?.kind === 'initialize' && (
        <InitializeProjectModal
          workspaceId={workspace.id}
          projectId={reviewing.projectId}
          onClose={() => {
            setReviewing(null);
            reload();
          }}
          onInitialized={reload}
        />
      )}
      {together && (
        <WorkspaceInitializeModal
          workspaceId={workspace.id}
          projects={together}
          onClose={() => {
            setTogether(null);
            reload();
          }}
          onDone={reload}
        />
      )}
      {reviewing?.kind === 'refresh' && (
        <RefreshHarnessModal
          workspaceId={workspace.id}
          projectId={reviewing.projectId}
          onClose={() => {
            setReviewing(null);
            reload();
          }}
          onApplied={reload}
        />
      )}
    </Modal>
  );
}

interface RowProps {
  info: ProjectInfoDto;
  state: ProjectHarnessStateDto;
  summary: HarnessSummaryDto | undefined;
  fixing: boolean;
  onFix: () => void;
  names: (id: string) => string;
  confirming: boolean;
  canRemove: boolean;
  canReset: boolean;
  confirmingReset: boolean;
  onAskReset: () => void;
  onCancelReset: () => void;
  onReset: () => void;
  onEdit: () => void;
  onReview: () => void;
  onUpdate: () => void;
  onAskRemove: () => void;
  onCancelRemove: () => void;
  onRemove: () => void;
}

function ProjectRow({
  info,
  state,
  summary,
  fixing,
  onFix,
  names,
  confirming,
  canRemove,
  canReset,
  confirmingReset,
  onAskReset,
  onCancelReset,
  onReset,
  onEdit,
  onReview,
  onUpdate,
  onAskRemove,
  onCancelRemove,
  onRemove,
}: RowProps) {
  const t = useT();
  const canStart = info.exists && (state === 'not_initialized' || state === 'needs_review');
  const canRefresh = state === 'ready' || state === 'outdated';
  return (
    <>
      <div className={projectStyles.rowHead}>
        <strong className={projectStyles.name}>{info.name}</strong>
        {info.primary && <span className={projectStyles.tag}>{t('projects.primary')}</span>}
        {info.role && <span className={projectStyles.tag}>{t(`projects.role.${info.role}`)}</span>}
        <span className={projectStyles.state} data-state={state}>
          {t(`projects.state.${state}`)}
        </span>
        <span className={projectStyles.rowActions}>
          {canRefresh && (
            <button
              type="button"
              className={styles.linkButton}
              onClick={onUpdate}
              aria-label={t('projects.updateHarness', { name: info.name })}
            >
              {t('harness.update')}
            </button>
          )}
          {(canStart || canRefresh) && (
            <button
              type="button"
              className={styles.linkButton}
              onClick={onReview}
              aria-label={
                canRefresh
                  ? t('projects.refreshHarness', { name: info.name })
                  : t('projects.openHarness', { name: info.name })
              }
            >
              {canRefresh
                ? t('harness.refresh')
                : state === 'needs_review'
                  ? t('harness.openReview')
                  : t('harness.initialize')}
            </button>
          )}
          {canReset && !confirmingReset && (
            <button
              type="button"
              className={styles.linkButton}
              aria-label={t('projects.reset', { name: info.name })}
              onClick={onAskReset}
            >
              {t('common.reset')}
            </button>
          )}
          <button
            type="button"
            className={styles.linkButton}
            aria-label={t('projects.edit', { name: info.name })}
            onClick={onEdit}
          >
            {t('common.edit')}
          </button>
          {canRemove &&
            (confirming ? (
              <span role="group" aria-label={t('projects.remove', { name: info.name })}>
                {t('projects.removeConfirm', { name: info.name })}{' '}
                <button type="button" className={styles.danger} onClick={onRemove}>
                  {t('projects.confirmRemove')}
                </button>{' '}
                <button type="button" className={styles.linkButton} onClick={onCancelRemove}>
                  {t('common.cancel')}
                </button>
              </span>
            ) : (
              <button
                type="button"
                className={styles.linkButton}
                aria-label={t('projects.remove', { name: info.name })}
                onClick={onAskRemove}
              >
                {t('common.delete')}
              </button>
            ))}
        </span>
      </div>
      <span className={projectStyles.path} title={info.path}>
        <Icon name="folder" size={12} /> {info.path}
      </span>
      {summary && state !== 'not_initialized' && state !== 'folder_missing' && (
        <div className={projectStyles.details}>
          {summary.health && <HealthBadge health={summary.health} />}
          {summary.stack.length > 0 && (
            <span className={projectStyles.muted}>{summary.stack.join(' · ')}</span>
          )}
          {summary.stats && <HarnessStats stats={summary.stats} analyzedAt={summary.analyzedAt} />}
          {summary.staleness && (
            <StaleNotice
              staleness={summary.staleness}
              fix={{ busy: fixing, error: null, onFix, onReview }}
            />
          )}
        </div>
      )}
      {confirmingReset && (
        <div role="group" aria-label={t('projects.reset', { name: info.name })}>
          <p className={projectStyles.problem}>{t('projects.resetConfirm', { name: info.name })}</p>
          <button type="button" className={styles.danger} onClick={onReset}>
            {t('projects.confirmReset')}
          </button>{' '}
          <button type="button" className={styles.linkButton} onClick={onCancelReset}>
            {t('common.cancel')}
          </button>
        </div>
      )}
      {!info.exists ? (
        <p className={projectStyles.problem}>{t('project.folderMissing')}</p>
      ) : (
        <p className={projectStyles.muted}>
          {info.technologies.length > 0
            ? info.technologies.join(' · ')
            : t('projects.noTechnologies')}
        </p>
      )}
      {info.dependsOn.length > 0 && (
        <p className={projectStyles.muted}>
          {t('projects.dependsOn', { names: info.dependsOn.map(names).join(', ') })}
        </p>
      )}
    </>
  );
}

interface ContextProps {
  workspace: Workspace;
  info: { exists: boolean; outdated: boolean };
  onRebuilt: () => void;
}

function WorkspaceContextSection({ workspace, info, onRebuilt }: ContextProps) {
  const t = useT();
  const [notes, setNotes] = useState<string | null>(null);
  const [message, setMessage] = useState<{ error: boolean; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const single = workspace.projects.length < 2;

  useEffect(() => {
    let cancelled = false;
    getWorkspaceNotes(workspace.id)
      .then((text) => {
        if (!cancelled) setNotes(text);
      })
      .catch(() => {
        if (!cancelled) setNotes('');
      });
    return () => {
      cancelled = true;
    };
  }, [workspace.id]);

  function rebuild() {
    setBusy(true);
    setMessage(null);
    buildWorkspaceContext(workspace.id)
      .catch((e: unknown) => {
        setMessage({ error: true, text: errorMessage(t, e) });
      })
      .finally(() => {
        setBusy(false);
        onRebuilt();
      });
  }

  function saveNotes() {
    if (notes === null) return;
    setBusy(true);
    setMessage(null);
    saveWorkspaceNotes(workspace.id, notes)
      .then(() => {
        setMessage({ error: false, text: t('projects.notes.saved') });
      })
      .catch((e: unknown) => {
        setMessage({ error: true, text: errorMessage(t, e) });
      })
      .finally(() => {
        setBusy(false);
      });
  }

  return (
    <section className={projectStyles.section} aria-label={t('projects.context.title')}>
      <h3 className={projectStyles.sectionTitle}>{t('projects.context.title')}</h3>
      {single ? (
        <p className={projectStyles.muted}>{t('projects.context.single')}</p>
      ) : (
        <>
          <p className={projectStyles.muted}>
            {!info.exists
              ? t('projects.context.none')
              : info.outdated
                ? t('projects.context.outdated')
                : t('projects.context.current')}
          </p>
          <div className={projectStyles.toolbar}>
            <Button variant="secondary" disabled={busy} onClick={rebuild}>
              {t('projects.context.rebuild')}
            </Button>
          </div>
        </>
      )}
      <label htmlFor={`notes-${workspace.id}`} className={projectStyles.name}>
        {t('projects.notes.title')}
      </label>
      <p className={projectStyles.muted}>{t('projects.notes.hint')}</p>
      <textarea
        id={`notes-${workspace.id}`}
        className={[styles.control, projectStyles.textarea].filter(Boolean).join(' ')}
        value={notes ?? ''}
        disabled={notes === null}
        onChange={(e) => {
          setNotes(e.target.value);
        }}
      />
      <div className={projectStyles.toolbar}>
        <Button variant="secondary" disabled={busy || notes === null} onClick={saveNotes}>
          {t('projects.notes.save')}
        </Button>
        {message && (
          <span
            role={message.error ? 'alert' : 'status'}
            className={message.error ? projectStyles.problem : projectStyles.muted}
          >
            {message.text}
          </span>
        )}
      </div>
    </section>
  );
}
