import { useEffect, useRef, useState } from 'react';
import { useNavigation } from '@/app/NavigationContext';
import { useNow } from '@/features/agents/hooks/useNow';
import { useCatalog } from '@/features/agents/hooks/useCatalog';
import { formatDuration } from '@/features/usage/model/format';
import { useI18n } from '@/i18n/I18nProvider';
import { useWorkspace } from '../hooks/WorkspaceProvider';
import { agentStatus } from '../model/agentStatus';
import styles from './Inspector.module.css';

/**
 * Always in the header while any agent is running, in any workspace. Lists each run with what
 * it is doing; choosing one goes to its agent (switching workspace if needed) without stopping
 * anything.
 */
export function ActiveExecutions() {
  const { t } = useI18n();
  const { catalog, runtimes } = useCatalog();
  const workspace = useWorkspace();
  const { navigate } = useNavigation();
  const now = useNow(1000);
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return undefined;
    function onPointerDown(event: MouseEvent) {
      if (root.current && !root.current.contains(event.target as Node)) setOpen(false);
    }
    document.addEventListener('mousedown', onPointerDown);
    return () => {
      document.removeEventListener('mousedown', onPointerDown);
    };
  }, [open]);

  if (catalog.status !== 'ready') return null;
  const items = Object.entries(workspace.conversations.runs)
    .filter(([, run]) => run.status === 'running')
    .flatMap(([key, run]) => {
      const [workspaceId = '', agentId = ''] = key.split('/');
      const agent = catalog.agents.find((a) => a.id === agentId);
      const owner = workspace.workspaces.find((w) => w.id === workspaceId);
      if (!agent || !owner) return [];
      const runtime =
        runtimes.status === 'ready'
          ? runtimes.runtimes.find((r) => r.runtime.id === agent.runtimeId)
          : undefined;
      const process = workspace.conversations.processes[run.executionId];
      return [{ key, run, agent, owner, status: agentStatus(run, runtime, process) }];
    });
  if (items.length === 0) return null;

  return (
    <div className={styles.active} ref={root}>
      <button
        type="button"
        className={styles.activeButton}
        aria-haspopup="true"
        aria-expanded={open}
        onClick={() => {
          setOpen((value) => !value);
        }}
      >
        <span className={styles.dot} aria-hidden="true">
          ●
        </span>
        {items.length === 1
          ? t('active.summaryOne')
          : t('active.summaryMany', { count: items.length })}
      </button>
      {open && (
        <ul className={styles.activePanel} aria-label={t('active.title')}>
          {items.map(({ key, run, agent, owner, status }) => (
            <li key={key}>
              <button
                type="button"
                className={styles.activeItem}
                aria-label={t('active.open', { agent: agent.name, workspace: owner.name })}
                onClick={() => {
                  setOpen(false);
                  navigate('workspace');
                  void workspace.focusAgent({
                    workspaceId: owner.id,
                    agentId: agent.id,
                    tab: 'chat',
                  });
                }}
              >
                <span>{agent.name}</span>
                <span className={styles.activeMeta}>
                  {t(`agent.status.${status}`)} · {formatDuration(now - run.startedAt)} ·{' '}
                  {owner.name}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
