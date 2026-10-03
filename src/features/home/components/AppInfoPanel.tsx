import { useI18n } from '@/i18n/I18nProvider';
import type { AppInfoState } from '../hooks/useAppInfo';
import styles from './AppInfoPanel.module.css';

export function AppInfoPanel({ state }: { state: AppInfoState }) {
  const { t } = useI18n();
  if (state.status === 'loading') {
    return <p className={styles.muted}>{t('common.loadingCore')}</p>;
  }
  if (state.status === 'error') {
    return (
      <p role="alert" className={styles.error}>
        {t('common.coreUnavailable', { message: state.message })}
      </p>
    );
  }
  const { name, version, platform } = state.info;
  return (
    <dl className={styles.list} aria-label="Application info">
      <dt>Atlas</dt>
      <dd>{name}</dd>
      <dt>{t('settings.version')}</dt>
      <dd>{version}</dd>
      <dt>{t('settings.platform')}</dt>
      <dd>{platform}</dd>
    </dl>
  );
}
