import { useEffect, useState } from 'react';
import type { TranslationKey } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import { api, ApiError } from './api';
import { Fold } from './Fold';
import type { RemoteSpecDetail, RemoteSpecSummary } from './types';
import styles from './Mobile.module.css';

type State =
  | { name: 'loading' }
  | { name: 'ready'; spec: RemoteSpecDetail }
  | { name: 'missing' }
  | { name: 'failed' };

/** The specifications, followed from the phone. Read only: deciding stays at the computer. */
export function SpecsSection() {
  const { t } = useI18n();
  const [specs, setSpecs] = useState<RemoteSpecSummary[]>([]);

  useEffect(() => {
    let current = true;
    api
      .specs()
      .then((found) => {
        if (current) setSpecs(found.specs);
      })
      .catch(() => {
        if (current) setSpecs([]);
      });
    return () => {
      current = false;
    };
  }, []);

  if (specs.length === 0) return null;
  return (
    <section className={styles.section}>
      <h2 className={styles.sectionTitle}>
        {t('mobile.specs.title')} <span className={styles.count}>{specs.length}</span>
      </h2>
      <ul className={styles.list}>
        {specs.map((spec) => (
          <li key={spec.id}>
            <a className={styles.card} href={`#/spec/${encodeURIComponent(spec.id)}`}>
              <span className={styles.cardTop}>
                <span className={styles.cardTitle}>{spec.title}</span>
              </span>
              <span className={styles.cardLine}>
                <span>{spec.workspaceName}</span>
                <span>{t(`specs.status.${spec.status}` as TranslationKey)}</span>
                <span>
                  {t('mobile.specs.approved', { done: spec.approved, total: spec.documents })}
                </span>
              </span>
            </a>
          </li>
        ))}
      </ul>
    </section>
  );
}

/** One specification: where each document stands and the text of its current revision. */
export function SpecScreen({ id }: { id: string }) {
  const { t } = useI18n();
  const [state, setState] = useState<State>({ name: 'loading' });

  useEffect(() => {
    let current = true;
    api
      .spec(id)
      .then((spec) => {
        if (current) setState({ name: 'ready', spec });
      })
      .catch((error: unknown) => {
        if (!current) return;
        setState(
          error instanceof ApiError && error.status === 404
            ? { name: 'missing' }
            : { name: 'failed' },
        );
      });
    return () => {
      current = false;
    };
  }, [id]);

  if (state.name === 'loading') {
    return (
      <p className={styles.empty} role="status">
        {t('mobile.loading')}
      </p>
    );
  }
  if (state.name !== 'ready') {
    return (
      <p className={styles.empty} role="alert">
        {t(state.name === 'missing' ? 'mobile.specs.notFound' : 'mobile.specs.failed')}
      </p>
    );
  }
  const { spec } = state;
  return (
    <article>
      <h1 className={styles.cardTitle}>{spec.title}</h1>
      <p className={styles.fine}>
        {spec.workspaceName} · {t(`specs.status.${spec.status}` as TranslationKey)}
      </p>
      <p className={styles.fine}>{spec.request}</p>
      {spec.documents.map((doc) => (
        <Fold
          key={doc.id}
          title={t(`specs.stage.${doc.id}` as TranslationKey)}
          line={[
            doc.phase ? t(`specs.phase.${doc.phase}` as TranslationKey) : '',
            doc.revision ? t('mobile.specs.revision', { number: doc.revision }) : '',
          ]
            .filter(Boolean)
            .join(' · ')}
        >
          {doc.files.length === 0 ? (
            <p className={styles.fine}>{t('mobile.specs.notWritten')}</p>
          ) : (
            doc.files.map((file) => (
              <div key={file.name}>
                {doc.files.length > 1 && <p className={styles.fine}>{file.name}</p>}
                <pre className={styles.pre}>{file.text}</pre>
              </div>
            ))
          )}
        </Fold>
      ))}
      <p className={styles.fine}>{t('mobile.specs.atComputer')}</p>
    </article>
  );
}
