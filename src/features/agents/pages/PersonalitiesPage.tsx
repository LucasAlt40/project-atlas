import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import { PersonalityBrowser } from '../components/PersonalityBrowser';
import { useCatalog } from '../hooks/useCatalog';
import styles from './Pages.module.css';

interface Props {
  /** The user chose "use this personality": go create an agent with it. */
  onUse: (personalityId: string) => void;
}

/** Personalities are global: they are shared by every agent and every workspace. */
export function PersonalitiesPage({ onUse }: Props) {
  const t = useT();
  const { catalog, addPersonality, editPersonality, removePersonality, restorePersonalities } =
    useCatalog();

  if (catalog.status === 'loading') {
    return <p className={styles.muted}>{t('common.loadingCore')}</p>;
  }
  if (catalog.status === 'error') {
    return (
      <p role="alert" className={styles.error}>
        {t('common.coreUnavailable', { message: errorMessage(t, catalog.error) })}
      </p>
    );
  }
  return (
    <section className={styles.page}>
      <h1 className={styles.title}>{t('personalities.title')}</h1>
      <PersonalityBrowser
        personalities={catalog.personalities}
        onUse={onUse}
        onCreate={addPersonality}
        onEdit={editPersonality}
        onDelete={removePersonality}
        onRestoreDefaults={restorePersonalities}
      />
    </section>
  );
}
