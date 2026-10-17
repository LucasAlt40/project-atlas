import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
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
      <div className={styles.welcome}>
        <div className={styles.welcomeHead}>
          <span className={styles.welcomeTile} aria-hidden="true">
            <Icon name="workspace" size={22} />
          </span>
          <div>
            <h1>{t('workspace.onboarding.title')}</h1>
            <p>{t('workspace.onboarding.text')}</p>
          </div>
        </div>
        <ol className={styles.steps}>
          {(
            [
              ['01', 'step1', 'folder'],
              ['02', 'step2', 'agents'],
              ['03', 'step3', 'shield'],
            ] as const
          ).map(([number, key, icon]) => (
            <li key={key} className={styles.step}>
              <span className={styles.stepHead}>
                <span className={styles.stepNumber}>{number}</span>
                <Icon name={icon} size={16} />
              </span>
              <h2>{t(`workspace.onboarding.${key}.title`)}</h2>
              <p>{t(`workspace.onboarding.${key}.text`)}</p>
            </li>
          ))}
        </ol>
        <div>
          <Button
            onClick={() => {
              setCreating(true);
            }}
          >
            {t('workspace.onboarding.cta')}
            <Icon name="arrowRight" size={14} />
          </Button>
        </div>
      </div>
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
