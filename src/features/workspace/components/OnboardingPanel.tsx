import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { useT } from '@/i18n/I18nProvider';
import { useWorkspace } from '../hooks/WorkspaceProvider';
import { WorkspaceForm } from './WorkspaceForm';
import styles from './Workspace.module.css';

interface Props {
  pickFolder?: (defaultPath?: string) => Promise<string | null>;
}

/** What a first-time user sees: no workspaces yet. */
export function OnboardingPanel({ pickFolder }: Props) {
  const t = useT();
  const { create } = useWorkspace();
  const [creating, setCreating] = useState(false);
  const close = () => {
    setCreating(false);
  };

  return (
    <section className={styles.onboarding}>
      <h1 className={styles.title}>{t('workspace.onboarding.title')}</h1>
      <p className={styles.muted}>{t('workspace.onboarding.text')}</p>
      <Button
        onClick={() => {
          setCreating(true);
        }}
      >
        {t('workspace.onboarding.cta')}
      </Button>
      {creating && (
        <Modal label={t('workspace.form.createTitle')} onClose={close}>
          <WorkspaceForm
            onSubmit={create}
            onSaved={close}
            onCancel={close}
            {...(pickFolder ? { pickFolder } : {})}
          />
        </Modal>
      )}
    </section>
  );
}
