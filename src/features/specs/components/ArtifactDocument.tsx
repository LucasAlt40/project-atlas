import { useState } from 'react';
import { Markdown } from '@/components/ui/Markdown';
import { useI18n } from '@/i18n/I18nProvider';
import { agoText } from '@/features/workflow/model/time';
import { stageName } from '../model/messages';
import type { SpecArtifact, SpecView } from '../types';
import { SpecsOutline } from './SpecsOutline';
import styles from './Specs.module.css';

interface Props {
  view: SpecView;
  artifact: SpecArtifact;
  /** Where a finding points: bring it into view. */
  focus?: string | null;
}

type Mode = 'rendered' | 'source';

/**
 * The document, as the thing the person reads: its file, its revision, and what the checks and
 * the review said about it are beside it (the inspector), not in the way.
 */
export function ArtifactDocument({ view, artifact, focus }: Props) {
  const { t, language } = useI18n();
  const latest = artifact.revisions.at(-1);
  const [revision, setRevision] = useState<number | null>(null);
  const [file, setFile] = useState<string | null>(null);
  const [mode, setMode] = useState<Mode>('rendered');

  const shown = artifact.revisions.find((r) => r.number === (revision ?? latest?.number));
  const files = shown?.files ?? [];
  const current = files.find((f) => f.path === file) ?? files[0];
  const isLatest = shown?.number === latest?.number;
  // Only the latest revision of the specs has an outline: older ones are read as written.
  const outline =
    artifact.id === 'specs' && isLatest
      ? view.outline.find((o) => o.path === current?.path)
      : undefined;
  const canRender = outline !== undefined;

  if (!shown || !current) return null;
  const approved = artifact.approvedRevision === shown.number;

  return (
    <section
      className={styles.document}
      aria-label={t('specs.doc.label', { stage: stageName(t, artifact.id) })}
    >
      <header className={styles.documentBar}>
        <div className={styles.documentName}>
          <span className={styles.mono}>{current.path}</span>
          <span className={styles.muted}>
            {t('specs.doc.revision', { number: shown.number })}
            {' · '}
            {t(`specs.origin.${shown.origin}`)}
            {' · '}
            {agoText(shown.createdAt, language)}
          </span>
        </div>
        <div className={styles.documentTools}>
          {approved && (
            <span className={styles.approvedMark}>{t('specs.doc.approvedRevision')}</span>
          )}
          {!isLatest && <span className={styles.olderMark}>{t('specs.doc.older')}</span>}
          {artifact.revisions.length > 1 && (
            <label className={styles.inline}>
              <span className={styles.srOnly}>{t('specs.doc.chooseRevision')}</span>
              <select
                value={shown.number}
                onChange={(event) => {
                  setRevision(Number(event.target.value));
                }}
              >
                {[...artifact.revisions].reverse().map((r) => (
                  <option key={r.number} value={r.number}>
                    {t('specs.doc.revision', { number: r.number })}
                    {r.number === latest?.number ? ` (${t('specs.doc.current')})` : ''}
                  </option>
                ))}
              </select>
            </label>
          )}
          {canRender && (
            <div className={styles.segmented} role="group" aria-label={t('specs.doc.view')}>
              {(['rendered', 'source'] as const).map((name) => (
                <button
                  key={name}
                  type="button"
                  aria-pressed={mode === name}
                  onClick={() => {
                    setMode(name);
                  }}
                >
                  {t(`specs.doc.${name}`)}
                </button>
              ))}
            </div>
          )}
        </div>
      </header>
      {files.length > 1 && (
        <div className={styles.fileTabs} role="tablist" aria-label={t('specs.doc.files')}>
          {files.map((f) => (
            <button
              key={f.path}
              type="button"
              role="tab"
              aria-selected={f.path === current.path}
              onClick={() => {
                setFile(f.path);
              }}
            >
              {f.path.replace(/^specs\//, '').replace(/\/spec\.md$/, '')}
            </button>
          ))}
        </div>
      )}
      <div className={styles.docPage}>
        {canRender && mode === 'rendered' ? (
          <SpecsOutline outline={outline} focus={focus} />
        ) : mode === 'source' || artifact.id === 'specs' ? (
          <pre className={styles.source}>{current.content}</pre>
        ) : (
          <Markdown>{current.content}</Markdown>
        )}
      </div>
    </section>
  );
}
