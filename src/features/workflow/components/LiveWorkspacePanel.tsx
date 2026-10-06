import { useState } from 'react';
import { Icon } from '@/components/ui/Icon';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import { errorMessage } from '@/i18n/messages';
import type { LiveFileDto } from '@/lib/tauri/commands';
import { useLiveText } from '../hooks/useLiveText';
import { useLiveWorkspace, type LiveWorkspace } from '../hooks/useLiveWorkspace';
import { headlineOf, STATUS_LABEL, type LiveModel } from '../model/live';
import { getLiveDiff, getLiveFile } from '../services/liveWorkspaceService';
import { getDiff } from '../services/workflowService';
import type { WorkflowRun } from '../types';
import { CodeView, DiffView } from './LiveCode';
import { LiveActivity, LiveFileTree } from './LiveFiles';
import styles from './LiveWorkspace.module.css';

type Tab = 'diff' | 'file';
const TABS: readonly Tab[] = ['diff', 'file'];

/**
 * The code of a run as it is being written: the files of its isolated worktree that differ from
 * where the run started, their current text and their real diff. While the run can still change
 * the worktree it says LIVE; once it is over (or the worktree is gone) it says REVIEW.
 *
 * It only looks. Deciding what happens to the code (apply, keep, discard) is the result panel's,
 * and nothing here can do it.
 */
export function LiveWorkspacePanel({ run }: { run: WorkflowRun }) {
  const hasWorktree = run.integration.worktreeExecutionId !== null;
  const live = useLiveWorkspace(hasWorktree ? run.id : null);
  return <LiveWorkspaceView run={run} live={live} />;
}

/**
 * The panel over a Live Workspace somebody else follows (the Review Workspace follows it once and
 * also reads its availability), so the worktree is read and watched a single time.
 */
export function LiveWorkspaceView({ run, live }: { run: WorkflowRun; live: LiveWorkspace }) {
  const { t } = useI18n();
  const hasWorktree = run.integration.worktreeExecutionId !== null;
  const [open, setOpen] = useState(true);
  const [selected, setSelected] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>('diff');
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());

  if (!hasWorktree) return null;
  if (live.load.status === 'loading') {
    return (
      <section className={styles.panel} aria-label={t('live.title')}>
        <p className={styles.note} role="status">
          {t('live.loading')}
        </p>
      </section>
    );
  }
  if (live.load.status === 'error') {
    return (
      <section className={styles.panel} aria-label={t('live.title')}>
        <p role="alert" className={styles.error}>
          {errorMessage(t, live.load.error)}
        </p>
      </section>
    );
  }
  const model = live.load.model;
  if (model === null) return null;
  const { state } = model;
  const headline = headlineOf(state, run);
  // A file that left the changes (reverted to the baseline) is no longer one to look at.
  const change = selected === null ? undefined : state.files.find((f) => f.path === selected);
  const current = selected !== null && change === undefined ? null : selected;

  return (
    <section className={styles.panel} aria-label={t('live.title')} data-mode={headline.mode}>
      <header className={styles.banner} data-tone={headline.tone}>
        <button
          type="button"
          className={styles.collapse}
          aria-expanded={open}
          aria-label={t(open ? 'live.collapse' : 'live.expand')}
          onClick={() => {
            setOpen((value) => !value);
          }}
        >
          <Icon name={open ? 'chevronDown' : 'chevronRight'} size={14} />
        </button>
        <span className={styles.mode} data-mode={headline.mode}>
          {t(`live.mode.${headline.mode}` as TranslationKey)}
        </span>
        <h3 className={styles.title}>{t('live.title')}</h3>
        <span className={styles.headline} role="status" data-tone={headline.tone}>
          <span className={styles.dot} aria-hidden="true" />
          {t(headline.label)}
        </span>
        <span className={styles.spacer} />
        {state.observation === 'polling' && (
          <span className={styles.muted}>{t('live.polling')}</span>
        )}
        <span className={styles.totals} aria-label={t('live.totals')}>
          {t('live.files.count', { n: state.filesChanged })} ·{' '}
          <span data-sign="add">+{state.additions}</span>{' '}
          <span data-sign="del">−{state.deletions}</span>
        </span>
        <button
          type="button"
          className={styles.iconButton}
          disabled={live.refreshing}
          aria-label={t('live.refresh')}
          title={t('live.refresh')}
          onClick={live.refresh}
        >
          <Icon name="refresh" size={14} />
        </button>
      </header>
      {open && (
        <>
          <p className={styles.scope}>
            {t('live.scope', {
              branch: state.branch,
              baseline: state.baselineRevision.slice(0, 7),
            })}
          </p>
          <div className={styles.body}>
            <div className={styles.side}>
              <LiveFileTree
                files={state.files}
                selected={current}
                collapsed={collapsed}
                onSelect={setSelected}
                onToggle={(folder) => {
                  setCollapsed((previous) => {
                    const next = new Set(previous);
                    if (!next.delete(folder)) next.add(folder);
                    return next;
                  });
                }}
              />
              <LiveActivity
                entries={live.activity}
                onOpen={(path) => {
                  if (state.files.some((f) => f.path === path)) setSelected(path);
                }}
              />
            </div>
            <div className={styles.viewer}>
              <div className={styles.viewerHead}>
                <span className={styles.viewerPath}>
                  {current ?? t('live.files.all')}
                  {change && (
                    <span className={styles.status} data-status={change.status}>
                      {t(STATUS_LABEL[change.status])}
                    </span>
                  )}
                </span>
                <div className={styles.tabs} role="tablist" aria-label={t('live.tabs')}>
                  {TABS.map((name) => (
                    <button
                      key={name}
                      type="button"
                      role="tab"
                      className={styles.tab}
                      aria-selected={tab === name}
                      onClick={() => {
                        setTab(name);
                      }}
                    >
                      {t(`live.tab.${name}` as TranslationKey)}
                    </button>
                  ))}
                </div>
              </div>
              <div className={styles.viewerBody}>
                {tab === 'diff' || current === null ? (
                  <DiffPane run={run} model={model} path={current} />
                ) : (
                  <FilePane run={run} model={model} path={current} />
                )}
              </div>
            </div>
          </div>
        </>
      )}
    </section>
  );
}

/** The worktree is there to read from. Otherwise the saved changes are all there is. */
const readable = (model: LiveModel) => model.state.availability === 'available';

function DiffPane({
  run,
  model,
  path,
}: {
  run: WorkflowRun;
  model: LiveModel;
  path: string | null;
}) {
  const { t } = useI18n();
  const live = readable(model);
  // One file is read again when that file changed; the whole diff when anything did.
  const stamp = path === null ? model.state.revision : (model.touched[path] ?? 0);
  const { text, refreshing } = useLiveText(
    `diff|${path ?? ''}`,
    `diff|${path ?? ''}|${live ? 'live' : 'saved'}|${String(stamp)}`,
    model.state.availability === 'invalid'
      ? null
      : () => (live ? getLiveDiff(run.id, path ?? undefined) : getDiff(run.id, path ?? undefined)),
  );
  if (model.state.availability === 'invalid') {
    return <p className={styles.note}>{t('live.pane.invalid')}</p>;
  }
  if (text.status === 'loading') {
    return (
      <p className={styles.note} role="status">
        {t('common.loadingCore')}
      </p>
    );
  }
  if (text.status === 'error') {
    return (
      <p role="alert" className={styles.error}>
        {errorMessage(t, text.error)}
      </p>
    );
  }
  return (
    <div data-refreshing={refreshing} className={styles.pane}>
      <DiffView text={text.data} path={path} />
    </div>
  );
}

function FilePane({ run, model, path }: { run: WorkflowRun; model: LiveModel; path: string }) {
  const { t } = useI18n();
  const entry = model.state.files.find((f) => f.path === path);
  const deleted = entry?.status === 'deleted';
  const stamp = model.touched[path] ?? 0;
  const enabled = readable(model) && !deleted;
  const { text, refreshing } = useLiveText<LiveFileDto>(
    `file|${path}`,
    `file|${path}|${String(stamp)}`,
    enabled ? () => getLiveFile(run.id, path) : null,
  );
  if (deleted) return <p className={styles.note}>{t('live.file.deleted')}</p>;
  if (!readable(model)) return <p className={styles.note}>{t('live.file.unavailable')}</p>;
  if (text.status === 'loading') {
    return (
      <p className={styles.note} role="status">
        {t('common.loadingCore')}
      </p>
    );
  }
  if (text.status === 'error') {
    return (
      <p role="alert" className={styles.error}>
        {errorMessage(t, text.error)}
      </p>
    );
  }
  return (
    <div data-refreshing={refreshing} className={styles.pane}>
      <CodeView file={text.data} />
    </div>
  );
}
