import { useI18n } from '@/i18n/I18nProvider';
import styles from './Workflow.module.css';

interface Props {
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
}

/** "Approve plans automatically": a standing instruction of the person, never of an agent. */
export function AutoPlansToggle({ checked, disabled, onChange }: Props) {
  const { t } = useI18n();
  return (
    <label className={styles.autoPlans} title={t('workflow.autoPlans.hint')}>
      <input
        type="checkbox"
        className={styles.switch}
        checked={checked}
        disabled={disabled}
        onChange={(e) => {
          onChange(e.target.checked);
        }}
      />
      <span>{t('workflow.autoPlans')}</span>
    </label>
  );
}
