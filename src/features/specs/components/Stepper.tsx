import { Icon } from '@/components/ui/Icon';
import { useI18n } from '@/i18n/I18nProvider';
import { phaseView, statusView } from '../model/status';
import { STAGES, type SpecView, type Stage } from '../types';
import { stageName } from '../model/messages';
import styles from './Specs.module.css';

interface Props {
  view: SpecView;
  selected: Stage;
  onSelect: (stage: Stage) => void;
}

/** Proposal → Requirements → Design → Tasks → Final review, each with where it stands. */
export function Stepper({ view, selected, onSelect }: Props) {
  const { t } = useI18n();
  return (
    <nav className={styles.stepper} aria-label={t('specs.stages')}>
      <ol>
        {STAGES.map((stage, index) => {
          const phase = stage === 'final' ? undefined : view.phases[stage];
          const tone = phase ? phaseView(phase).tone : statusView(view.status).tone;
          const label = phase
            ? t(phaseView(phase).key)
            : view.readiness.ready
              ? t('specs.phase.approved')
              : t('specs.stage.finalOpen');
          const done = phase === 'approved' || (stage === 'final' && view.readiness.ready);
          return (
            <li key={stage}>
              <button
                type="button"
                className={styles.step}
                data-tone={tone}
                aria-current={selected === stage ? 'step' : undefined}
                onClick={() => {
                  onSelect(stage);
                }}
              >
                <span className={styles.stepText}>
                  <span className={styles.stepName}>
                    {index + 1}. {stageName(t, stage)}
                  </span>
                  <span className={styles.stepState}>{label}</span>
                </span>
                <span className={styles.stepMark} aria-hidden="true">
                  {done ? (
                    <Icon name="check" size={13} />
                  ) : phase === 'blocked' ? (
                    <Icon name="lock" size={13} />
                  ) : null}
                </span>
              </button>
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
