import { useDismiss } from '@/hooks/useDismiss';
import { useRef, useState } from 'react';
import { Badge } from '@/components/ui/Badge';
import { Icon } from '@/components/ui/Icon';
import { Modal } from '@/components/ui/Modal';
import { useT } from '@/i18n/I18nProvider';
import { useWorkspace } from '../hooks/WorkspaceProvider';
import { addWorkspaceProject } from '../services/workspaceService';
import type { Workspace } from '../types';
import { ManageWorkspaces } from './ManageWorkspaces';
import { WorkspaceForm } from './WorkspaceForm';
import styles from './Workspace.module.css';

type Dialog = { type: 'new' } | { type: 'manage' } | { type: 'edit'; workspace: Workspace };

interface Props {
  /** Opens the folder picker; defaults to the native one. Injectable for tests. */
  pickFolder?: (defaultPath?: string) => Promise<string | null>;
}

/**
 * The current workspace, always visible in the shell: a dropdown to switch, create or manage
 * workspaces. Switching only changes what is shown; agents that are running keep running.
 */
export function WorkspaceSwitcher({ pickFolder }: Props) {
  const t = useT();
  const { workspaces, active, select, create, update, remove, replaceWorkspace } = useWorkspace();
  const [open, setOpen] = useState(false);
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const root = useRef<HTMLDivElement>(null);

  useDismiss(open, root, () => {
    setOpen(false);
  });

  const closeDialog = () => {
    setDialog(null);
  };

  return (
    <div className={styles.switcher} ref={root}>
      <button
        type="button"
        className={styles.switcherButton}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={`${t('workspace.switcher.label')}: ${active?.name ?? t('workspace.switcher.none')}`}
        onClick={() => {
          setOpen((o) => !o);
        }}
      >
        <span className={styles.switcherText}>
          <span className={styles.switcherName}>
            {active?.name ?? t('workspace.switcher.none')}
          </span>
          {active && (
            <span className={styles.switcherPath} title={active.projectPath}>
              {active.projectPath}
            </span>
          )}
        </span>
        <Icon name="unfold" size={16} />
      </button>

      {open && (
        <div className={styles.menu} role="listbox" aria-label={t('workspace.switcher.label')}>
          <p className={styles.menuHeading} aria-hidden="true">
            <span>{t('workspace.switcher.heading')}</span>
            <span>{workspaces.length}</span>
          </p>
          {workspaces.map((workspace) => {
            const isActive = workspace.id === active?.id;
            return (
              <button
                key={workspace.id}
                type="button"
                role="option"
                aria-selected={isActive}
                className={styles.menuItem}
                onClick={() => {
                  setOpen(false);
                  void select(workspace.id);
                }}
              >
                <span className={styles.menuText}>
                  <span className={styles.menuLine}>
                    <strong className={styles.menuName}>{workspace.name}</strong>
                    {isActive && <Badge tone="accent">{t('workspace.switcher.active')}</Badge>}
                  </span>
                  <span className={styles.menuPath}>{workspace.projectPath}</span>
                </span>
                {isActive && (
                  <span className={styles.menuCheck} aria-hidden="true">
                    <Icon name="check" size={16} />
                  </span>
                )}
              </button>
            );
          })}
          {workspaces.length > 0 && <hr className={styles.menuRule} />}
          <button
            type="button"
            className={styles.menuAction}
            onClick={() => {
              setOpen(false);
              setDialog({ type: 'new' });
            }}
          >
            <Icon name="plus" size={14} />
            {t('workspace.switcher.new')}
          </button>
          <button
            type="button"
            className={styles.menuAction}
            onClick={() => {
              setOpen(false);
              setDialog({ type: 'manage' });
            }}
          >
            {t('workspace.switcher.manage')}
            <Icon name="arrowRight" size={14} />
          </button>
        </div>
      )}

      {dialog?.type === 'new' && (
        <Modal label={t('workspace.form.createTitle')} onClose={closeDialog}>
          <WorkspaceForm
            onSubmit={create}
            onAddProject={async (workspaceId, path) => {
              const updated = await addWorkspaceProject(workspaceId, { path });
              replaceWorkspace(updated);
              return updated;
            }}
            onSaved={closeDialog}
            onCancel={closeDialog}
            {...(pickFolder ? { pickFolder } : {})}
          />
        </Modal>
      )}
      {dialog?.type === 'edit' && (
        <Modal label={t('workspace.form.editTitle')} onClose={closeDialog}>
          <WorkspaceForm
            initial={dialog.workspace}
            onSubmit={(input) => update(dialog.workspace.id, input)}
            onSaved={closeDialog}
            onCancel={closeDialog}
            {...(pickFolder ? { pickFolder } : {})}
          />
        </Modal>
      )}
      {dialog?.type === 'manage' && (
        <Modal label={t('workspace.manage.title')} onClose={closeDialog}>
          <ManageWorkspaces
            workspaces={workspaces}
            onEdit={(workspace) => {
              setDialog({ type: 'edit', workspace });
            }}
            onDelete={remove}
          />
        </Modal>
      )}
    </div>
  );
}
