import { useState } from 'react';
import { Modal } from '@/components/ui/Modal';
import { useT } from '@/i18n/I18nProvider';
import { useWorkspaceSecurity } from '../hooks/useSecurity';
import type { Workspace } from '../types';
import { PolicySummary } from './PolicySummary';
import styles from './Security.module.css';

/**
 * The workspace's security at a glance ("Secure mode" / "Developer permissions"); clicking it
 * explains what agents can do here and, as importantly, what Atlas cannot control. Everything
 * shown is read from the core, which is also what enforces it.
 */
export function SecurityBadge({ workspace }: { workspace: Workspace }) {
  const t = useT();
  const security = useWorkspaceSecurity(workspace.id, workspace.projectPath);
  const [open, setOpen] = useState(false);
  if (security.status !== 'ready') return null;
  const { label, policy, projectPath } = security.value;

  return (
    <>
      <button
        type="button"
        className={styles.badge}
        data-mode={label}
        aria-label={t('security.title')}
        onClick={() => {
          setOpen(true);
        }}
      >
        <span aria-hidden="true">🔒</span> {t(`security.label.${label}`)}
      </button>
      {open && (
        <Modal
          label={t('security.title')}
          onClose={() => {
            setOpen(false);
          }}
        >
          <h2 className={styles.title}>{t('security.title')}</h2>
          <p className={styles.path} title={projectPath}>
            {projectPath}
          </p>
          <PolicySummary policy={policy} showNetworkNote />
          <p className={styles.note}>{t('security.note.workspace')}</p>
          <p className={styles.note}>{t('security.note.limits')}</p>
        </Modal>
      )}
    </>
  );
}
