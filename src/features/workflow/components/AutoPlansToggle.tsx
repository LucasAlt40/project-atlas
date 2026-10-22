import { Switch } from '@/components/ui/Switch';
import { useI18n } from '@/i18n/I18nProvider';

interface Props {
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
}

/** "Approve plans automatically": a standing instruction of the person, never of an agent. */
export function AutoPlansToggle({ checked, disabled, onChange }: Props) {
  const { t } = useI18n();
  return (
    <Switch
      checked={checked}
      disabled={disabled === true}
      title={t('workflow.autoPlans.hint')}
      onChange={onChange}
    >
      {t('workflow.autoPlans')}
    </Switch>
  );
}
