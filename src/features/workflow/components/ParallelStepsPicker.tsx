import { useI18n } from '@/i18n/I18nProvider';
import styles from './ParallelStepsPicker.module.css';

/** What the core accepts (`MAX_PARALLEL_STEPS_RANGE`): one step at a time, up to eight. */
export const PARALLEL_STEPS = [1, 2, 3, 4, 5, 6, 7, 8] as const;

interface Props {
  value: number;
  disabled?: boolean;
  onChange: (steps: number) => void;
}

/** How many steps of the run may run at once: a setting of the person, for the next steps. */
export function ParallelStepsPicker({ value, disabled, onChange }: Props) {
  const { t } = useI18n();
  return (
    <label className={styles.picker} title={t('workflow.parallel.hint')}>
      <span>{t('workflow.parallel')}</span>
      <select
        className={styles.control}
        value={value}
        disabled={disabled === true}
        onChange={(e) => {
          onChange(Number(e.target.value));
        }}
      >
        {PARALLEL_STEPS.map((n) => (
          <option key={n} value={n}>
            {n}
          </option>
        ))}
      </select>
    </label>
  );
}
