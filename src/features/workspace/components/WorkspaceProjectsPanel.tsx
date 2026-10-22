import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { Badge } from '@/components/ui/Badge';
import { Callout } from '@/components/ui/Callout';
import { PageHeader } from '@/components/ui/PageHeader';
import { StateBadge, type StateKind } from '@/components/ui/status';
import { useBreadcrumb } from '@/app/BreadcrumbContext';
import { OverflowMenu } from '@/components/ui/OverflowMenu';
import { InitializeProjectModal } from '@/features/harness/components/InitializeProjectModal';
import { HarnessStats } from '@/features/harness/components/HarnessStats';
import { HealthBadge } from '@/features/harness/components/HealthBadge';
import { RefreshHarnessModal } from '@/features/harness/components/RefreshHarnessModal';
import { StaleNotice, formatAnalyzed } from '@/features/harness/components/StaleNotice';
import {
  WorkspaceInitializeModal,
  type ProjectRef,
} from '@/features/harness/components/WorkspaceInitializeModal';
import { useI18n, useT } from '@/i18n/I18nProvider';
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

const PROJECT_STATE: Record<ProjectHarnessStateDto, StateKind> = {
  ready: 'completed',
  outdated: 'stale',
  needs_review: 'waiting',
  not_initialized: 'pending',
  folder_missing: 'failed',
};

/**
 * The page of a workspace's projects: add or remove folders, see where each project's Harness stands,
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
  useBreadcrumb([t('projects.crumb')]);
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

  // The flow of the Harness takes the page itself, with its three steps on top.
  if (together) {
    return (
      <WorkspaceInitializeModal
        presentation="page"
        workspaceId={workspace.id}
        projects={together}
        onClose={() => {
          setTogether(null);
          reload();
        }}
        onDone={reload}
      />
    );
  }
  if (reviewing?.kind === 'initialize') {
    return (
      <InitializeProjectModal
        presentation="page"
        workspaceId={workspace.id}
        projectId={reviewing.projectId}
        onClose={() => {
          setReviewing(null);
          reload();
        }}
        onInitialized={reload}
      />
    );
  }

  return (
    <section
      className={projectStyles.page}
      aria-label={t('projects.title', { name: workspace.name })}
    >
      <PageHeader
        back={{ label: t('projects.backShort'), onClick: onClose }}
        title={t('projects.title', { name: workspace.name })}
        description={t('projects.intro')}
        actions={
          <Button onClick={() => void addFolder()}>
            <Icon name="plus" size={16} />
            {t('projects.add')}
          </Button>
        }
      />
      {notice && (
        <Callout tone="warning" role="alert">
          {notice}
        </Callout>
      )}

      {state.status === 'loading' && <p className={projectStyles.muted}>…</p>}
      {state.status === 'error' && (
        <Callout tone="danger" role="alert">
          {t('projects.loadFailed')}
        </Callout>
      )}
      {ready && (
        <>
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

          <div className={projectStyles.cards}>
            <section className={projectStyles.card} aria-label={t('harness.title')}>
              <h3 className={projectStyles.cardTitle}>{t('harness.title')}</h3>
              <div className={projectStyles.cardRow}>
                <div className={projectStyles.cardText}>
                  <p className={projectStyles.muted}>{t('projects.init.note')}</p>
                  {pending.length === 0 && (
                    <p className={projectStyles.muted}>{t('projects.init.none')}</p>
                  )}
                </div>
                <div className={projectStyles.cardActions}>
                  {pending.length > 0 && (
                    <Button
                      disabled={starting}
                      onClick={() => {
                        setTogether(pending.map(({ id, name, path }) => ({ id, name, path })));
                      }}
                    >
                      {t('projects.init.all', { count: pending.length })}
                    </Button>
                  )}
                  {pending.length < ready.projects.length && confirmingReset !== 'all' && (
                    <Button
                      variant="secondary"
                      disabled={starting}
                      onClick={() => {
                        setConfirmingReset('all');
                      }}
                    >
                      {t('projects.resetAll')}
                    </Button>
                  )}
                </div>
              </div>
              {pending.length < ready.projects.length && confirmingReset === 'all' && (
                <div
                  role="group"
                  aria-label={t('projects.resetAll')}
                  className={projectStyles.confirm}
                >
                  <p className={projectStyles.problem}>{t('projects.resetAllConfirm')}</p>
                  <div className={projectStyles.cardActions}>
                    <Button
                      variant="ghost"
                      onClick={() => {
                        setConfirmingReset(null);
                      }}
                    >
                      {t('common.cancel')}
                    </Button>
                    <Button
                      variant="danger"
                      disabled={starting}
                      onClick={() => {
                        reset('all');
                      }}
                    >
                      {t('projects.confirmReset')}
                    </Button>
                  </div>
                </div>
              )}
            </section>

            <WorkspaceContextSection
              workspace={workspace}
              info={ready.overview.context}
              onRebuilt={reload}
            />
          </div>
        </>
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
    </section>
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
  const { t, language } = useI18n();
  const canStart = info.exists && (state === 'not_initialized' || state === 'needs_review');
  const canRefresh = state === 'ready' || state === 'outdated';
  const hasHarness = summary && state !== 'not_initialized' && state !== 'folder_missing';
  const menu = [
    ...(canRefresh
      ? [
          {
            label: t('harness.refresh'),
            icon: 'undo' as const,
            onSelect: onReview,
          },
        ]
      : []),
    ...(canReset
      ? [
          {
            label: t('projects.reset', { name: info.name }),
            icon: 'refresh' as const,
            danger: true,
            onSelect: onAskReset,
          },
        ]
      : []),
    ...(canRemove
      ? [
          {
            label: t('projects.remove', { name: info.name }),
            icon: 'trash' as const,
            danger: true,
            onSelect: onAskRemove,
          },
        ]
      : []),
  ];
  const stack =
    hasHarness && summary.stack.length > 0
      ? summary.stack.join(', ')
      : info.technologies.length > 0
        ? info.technologies.join(', ')
        : t('projects.noTechnologies');
  return (
    <>
      <div className={projectStyles.rowMain}>
        <div className={projectStyles.identity}>
          <div className={projectStyles.rowHead}>
            <strong className={projectStyles.name}>{info.name}</strong>
            {info.primary && <Badge>{t('projects.primary')}</Badge>}
            {info.role && <Badge>{t(`projects.role.${info.role}`)}</Badge>}
            <StateBadge state={PROJECT_STATE[state]}>{t(`projects.state.${state}`)}</StateBadge>
          </div>
          <span className={projectStyles.path} title={info.path}>
            {info.path}
          </span>
          {!info.exists ? (
            <p className={projectStyles.problem}>{t('project.folderMissing')}</p>
          ) : (
            <p className={projectStyles.line}>
              {hasHarness && summary.health && (
                <>
                  <HealthBadge health={summary.health} plain />
                  <span aria-hidden="true"> · </span>
                </>
              )}
              {stack}
            </p>
          )}
          {hasHarness && summary.stats && (
            <span className={projectStyles.statsLine}>
              <HarnessStats
                stats={summary.stats}
                analyzedAt={summary.analyzedAt}
                showDate={false}
                bar={false}
              />
            </span>
          )}
          <p className={projectStyles.meta}>
            {hasHarness && summary.analyzedAt ? (
              <span>
                {t('projects.analyzedAt', { date: formatAnalyzed(summary.analyzedAt, language) })}
              </span>
            ) : (
              <span>{t('projects.neverAnalyzed')}</span>
            )}
          </p>
          {info.dependsOn.length > 0 && (
            <p className={projectStyles.line}>
              {t('projects.dependsOn', { names: info.dependsOn.map(names).join(', ') })}
            </p>
          )}
        </div>
        <span className={projectStyles.rowActions}>
          {canRefresh && (
            <Button
              variant="secondary"
              onClick={onUpdate}
              aria-label={t('projects.updateHarness', { name: info.name })}
            >
              {t('harness.update')}
            </Button>
          )}
          {canStart && (
            <Button onClick={onReview} aria-label={t('projects.openHarness', { name: info.name })}>
              {state === 'needs_review' ? t('harness.openReview') : t('harness.initialize')}
            </Button>
          )}
          <Button
            variant="ghost"
            aria-label={t('projects.edit', { name: info.name })}
            onClick={onEdit}
          >
            {t('common.edit')}
          </Button>
          {menu.length > 0 && (
            <OverflowMenu label={t('projects.moreActions', { name: info.name })} items={menu} />
          )}
        </span>
      </div>
      {hasHarness && summary.staleness && (
        <StaleNotice
          staleness={summary.staleness}
          fix={{ busy: fixing, error: null, onFix, onReview }}
        />
      )}
      {confirming && (
        <div
          role="group"
          aria-label={t('projects.remove', { name: info.name })}
          className={projectStyles.confirm}
        >
          <p className={projectStyles.problem}>
            {t('projects.removeConfirm', { name: info.name })}
          </p>
          <div className={projectStyles.cardActions}>
            <Button variant="danger" onClick={onRemove}>
              {t('projects.confirmRemove')}
            </Button>
            <Button variant="ghost" onClick={onCancelRemove}>
              {t('common.cancel')}
            </Button>
          </div>
        </div>
      )}
      {confirmingReset && (
        <div
          role="group"
          aria-label={t('projects.reset', { name: info.name })}
          className={projectStyles.confirm}
        >
          <p className={projectStyles.problem}>{t('projects.resetConfirm', { name: info.name })}</p>
          <div className={projectStyles.cardActions}>
            <Button variant="danger" onClick={onReset}>
              {t('projects.confirmReset')}
            </Button>
            <Button variant="ghost" onClick={onCancelReset}>
              {t('common.cancel')}
            </Button>
          </div>
        </div>
      )}
    </>
  );
}

interface ContextProps {
  workspace: Workspace;
  info: { exists: boolean; outdated: boolean };
  onRebuilt: () => void;
}

/** The context that relates the projects, and the workspace's own notes: two cards of the page. */
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

  const contextState = !info.exists
    ? t('projects.context.none')
    : info.outdated
      ? t('projects.context.outdated')
      : t('projects.context.current');

  return (
    <>
      <section className={projectStyles.card} aria-label={t('projects.context.title')}>
        <h3 className={projectStyles.cardTitle}>{t('projects.context.title')}</h3>
        {single ? (
          <p className={projectStyles.muted}>{t('projects.context.single')}</p>
        ) : (
          <div className={projectStyles.cardRow}>
            <div className={projectStyles.cardText}>
              {info.exists ? (
                <StateBadge state={info.outdated ? 'stale' : 'completed'}>
                  {info.outdated
                    ? t('projects.context.badge.outdated')
                    : t('projects.context.badge.current')}
                </StateBadge>
              ) : null}
              {(!info.exists || info.outdated) && (
                <p className={projectStyles.muted}>{contextState}</p>
              )}
            </div>
            <div className={projectStyles.cardActions}>
              <Button variant="secondary" disabled={busy} onClick={rebuild}>
                {t('projects.context.rebuild')}
              </Button>
            </div>
          </div>
        )}
      </section>
      <section className={projectStyles.card}>
        <h3 className={projectStyles.cardTitle}>
          <label htmlFor={`notes-${workspace.id}`}>{t('projects.notes.title')}</label>
        </h3>
        <p className={projectStyles.muted}>{t('projects.notes.hint')}</p>
        <textarea
          id={`notes-${workspace.id}`}
          className={projectStyles.textarea}
          value={notes ?? ''}
          disabled={notes === null}
          onChange={(e) => {
            setNotes(e.target.value);
          }}
        />
        <div className={projectStyles.cardFoot}>
          {message && (
            <span
              role={message.error ? 'alert' : 'status'}
              className={message.error ? projectStyles.problem : projectStyles.muted}
            >
              {message.text}
            </span>
          )}
          <Button variant="secondary" disabled={busy || notes === null} onClick={saveNotes}>
            {t('projects.notes.save')}
          </Button>
        </div>
      </section>
    </>
  );
}
