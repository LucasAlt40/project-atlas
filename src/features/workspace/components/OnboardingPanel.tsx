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

const ATLAS_DIR = '.atlas/';
const SLOT = '\u0000';

/** What a first-time user sees: no workspaces yet, what one is and the three steps after it. */
export function OnboardingPanel({ pickFolder }: Props) {
  const t = useT();
  const { create } = useWorkspace();
  const [creating, setCreating] = useState(false);
  const close = () => {
    setCreating(false);
  };
  // The folder name is set in mono inside the sentence.
  const [before, after] = t('workspace.onboarding.step3.text', { path: SLOT }).split(SLOT);

  return (
    <section className={styles.onboarding}>
      <div className={styles.welcome}>
        <header className={styles.welcomeHead}>
          <h1>{t('workspace.onboarding.title')}</h1>
          <p>{t('workspace.onboarding.text')}</p>
        </header>
        <ol className={styles.steps}>
          {(['step1', 'step2', 'step3'] as const).map((key, index) => (
            <li key={key} className={styles.step}>
              <span className={styles.stepNumber} aria-hidden="true">
                {index + 1}
              </span>
              <div>
                <h2>{t(`workspace.onboarding.${key}.title`)}</h2>
                <p>
                  {key === 'step3' ? (
                    <>
                      {before}
                      <code>{ATLAS_DIR}</code>
                      {after}
                    </>
                  ) : (
                    t(`workspace.onboarding.${key}.text`)
                  )}
                </p>
              </div>
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
            <Icon name="arrowRight" size={16} />
          </Button>
        </div>
      </div>
      {creating && (
        <Modal
          label={t('workspace.form.createTitle')}
          title={t('workspace.form.createTitle')}
          onClose={close}
        >
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
