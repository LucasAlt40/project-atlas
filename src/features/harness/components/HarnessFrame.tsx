import type { ReactNode } from 'react';
import { Modal } from '@/components/ui/Modal';
import { PageHeader } from '@/components/ui/PageHeader';
import { Steps, type Step } from '@/components/ui/Steps';
import { useT } from '@/i18n/I18nProvider';
import rv from './Review.module.css';

export type HarnessPresentation = 'modal' | 'page';

/** 1 analyze, 2 live analysis (an agent reads the project), 3 review the context. */
export type HarnessStep = 1 | 2 | 3;

/** The three steps of the Harness flow, as a numbered line. */
export function HarnessSteps({
  step,
  finished = false,
}: {
  step: HarnessStep;
  finished?: boolean;
}) {
  const t = useT();
  const names = [t('harness.steps.analyze'), t('harness.steps.live'), t('harness.steps.review')];
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
    <div className={rv.pageSteps}>
      <Steps label={t('harness.steps.label')} steps={steps} />
    </div>
  );
}

interface Props {
  label: string;
  onClose: () => void;
  /** `page` takes the area of the screen under the top bar, with the steps on top. */
  presentation: HarnessPresentation;
  /** The modal's width. */
  size: 'normal' | 'analyze' | 'wide';
  step: HarnessStep;
  /** The last step is over, not just reached. */
  finished?: boolean;
  /** The page's title ("Analisar transport-web") and what sits next to it. */
  title?: ReactNode;
  titleAside?: ReactNode;
  /** Where "←" goes: back to the projects, or a step back. */
  back?: { label: string; onClick: () => void };
  /** The step's main action, at the right of the title. */
  actions?: ReactNode;
  children: ReactNode;
}

/**
 * Where a step of the Harness flow is drawn: in the page itself, with a "←", the title and the
 * three steps on top (or, for older callers, over the screen). The state and the steps
 * themselves do not depend on it.
 */
export function HarnessFrame({
  label,
  onClose,
  presentation,
  size,
  step,
  finished = false,
  title,
  titleAside,
  back,
  actions,
  children,
}: Props) {
  if (presentation === 'modal') {
    return (
      <Modal label={label} onClose={onClose} size={size === 'wide' ? 'lg' : 'md'}>
        {title !== undefined && <h2 className={rv.modalTitle}>{title}</h2>}
        {children}
      </Modal>
    );
  }
  return (
    <section className={rv.page} aria-label={label} data-step={step}>
      {title !== undefined && (
        <PageHeader
          title={title}
          {...(titleAside !== undefined ? { titleAside } : {})}
          {...(back ? { back } : {})}
          {...(actions !== undefined ? { actions } : {})}
        />
      )}
      <HarnessSteps step={step} finished={finished} />
      {children}
    </section>
  );
}
