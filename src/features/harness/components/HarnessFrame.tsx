import type { ReactNode } from 'react';
import { Modal } from '@/components/ui/Modal';
import { Steps, type Step } from '@/components/ui/Steps';
import { useT } from '@/i18n/I18nProvider';
import rv from './Review.module.css';

export type HarnessPresentation = 'modal' | 'page';

interface Props {
  label: string;
  onClose: () => void;
  /** `page` takes the area of the screen under the top bar, with the steps on top. */
  presentation: HarnessPresentation;
  /** The modal's width. */
  size: 'normal' | 'analyze' | 'wide';
  /** The step the flow is at: 1 analyze, 2 review, 3 initialize. */
  step: 1 | 2 | 3;
  /** The last step is over, not just reached. */
  finished?: boolean;
  children: ReactNode;
}

/**
 * Where a step of the Harness flow is drawn: over the screen, or (as the workspace's Projects
 * page does) in the page itself with the three steps of the flow shown. The state and the steps
 * themselves do not depend on it.
 */
export function HarnessFrame({
  label,
  onClose,
  presentation,
  size,
  step,
  finished = false,
  children,
}: Props) {
  const t = useT();
  if (presentation === 'modal') {
    return (
      <Modal label={label} onClose={onClose} size={size === 'wide' ? 'lg' : 'md'}>
        {children}
      </Modal>
    );
  }
  const names = [
    t('harness.steps.analyze'),
    t('harness.steps.review'),
    t('harness.steps.initialize'),
  ];
  const steps: Step[] = names.map((name, index) => ({
    label: name,
    state:
      index + 1 < step || (index + 1 === step && finished)
        ? 'done'
        : index + 1 === step
          ? 'current'
          : 'todo',
  }));
  return (
    <section className={rv.page} aria-label={label} data-step={step}>
      <div className={rv.pageSteps}>
        <Steps label={t('harness.steps.label')} steps={steps} />
      </div>
      {children}
    </section>
  );
}
