import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Markdown } from '@/components/ui/Markdown';
import { SegmentedControl } from '@/components/ui/SegmentedControl';
import { useI18n } from '@/i18n/I18nProvider';
import type { SpecFileDto } from '@/lib/tauri/commands';
import { agoText } from '@/features/workflow/model/time';
import { stageName } from '../model/messages';
import type { SpecArtifact, SpecView } from '../types';
import { RevisionDiff } from './RevisionDiff';
import { SpecsOutline } from './SpecsOutline';
import styles from './Specs.module.css';

interface Props {
  view: SpecView;
  artifact: SpecArtifact;
  /** Where a finding points: bring it into view. */
  focus?: string | null;
  /**
   * Saves the person's own text as the next revision. Absent while the document cannot be edited
   * (a run is going, the spec is archived). Resolves `true` when it was saved.
   */
  onEdit?: ((revision: number, files: SpecFileDto[]) => Promise<boolean>) | undefined;
}

type Mode = 'rendered' | 'source';

/**
 * The document, as the thing the person reads: its file, its revision, and what the checks and
 * the review said about it are beside it (the inspector), not in the way.
 */
export function ArtifactDocument({ view, artifact, focus, onEdit }: Props) {
  const { t, language } = useI18n();
  const latest = artifact.revisions.at(-1);
  const [revision, setRevision] = useState<number | null>(null);
  const [file, setFile] = useState<string | null>(null);
  const [mode, setMode] = useState<Mode>('rendered');
  /** Another revision to compare the one shown with; `null` shows the document itself. */
  const [compareWith, setCompareWith] = useState<number | null>(null);
  /** The text being edited, for the file shown; `null` when not editing. */
  const [draft, setDraft] = useState<string | null>(null);

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
          {artifact.revisions.length > 1 && (
            <label className={styles.inline}>
              <span className={styles.srOnly}>{t('specs.diff.compareWith')}</span>
              <select
                value={compareWith ?? ''}
                onChange={(event) => {
                  setCompareWith(event.target.value === '' ? null : Number(event.target.value));
                }}
              >
                <option value="">{t('specs.diff.none')}</option>
                {[...artifact.revisions]
                  .reverse()
                  .filter((r) => r.number !== shown.number)
                  .map((r) => (
                    <option key={r.number} value={r.number}>
                      {t('specs.diff.against', { number: r.number })}
                    </option>
                  ))}
              </select>
            </label>
          )}
          {onEdit && isLatest && draft === null && (
            <Button
              size="sm"
              variant="ghost"
              onClick={() => {
                setCompareWith(null);
                setDraft(current.content);
              }}
            >
              {t('specs.edit.start')}
            </Button>
          )}
          {canRender && compareWith === null && (
            <SegmentedControl
              label={t('specs.doc.view')}
              value={mode}
              onChange={setMode}
              segments={(['rendered', 'source'] as const).map((name) => ({
                value: name,
                label: t(`specs.doc.${name}`),
              }))}
            />
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
        {draft !== null && onEdit ? (
          <form
            className={styles.editor}
            aria-label={t('specs.edit.label', { file: current.path })}
            onSubmit={(event) => {
              event.preventDefault();
              const files = shown.files.map((f) =>
                f.path === current.path ? { path: f.path, content: draft } : f,
              );
              void onEdit(shown.number, files).then((saved) => {
                if (saved) setDraft(null);
              });
            }}
          >
            <p className={styles.muted}>{t('specs.edit.hint')}</p>
            <textarea
              aria-label={current.path}
              value={draft}
              spellCheck={false}
              onChange={(event) => {
                setDraft(event.target.value);
              }}
            />
            <div className={styles.inline}>
              <Button type="submit" disabled={draft.trim() === '' || draft === current.content}>
                {t('specs.edit.save')}
              </Button>
              <Button
                type="button"
                variant="ghost"
                onClick={() => {
                  setDraft(null);
                }}
              >
                {t('specs.edit.cancel')}
              </Button>
            </div>
          </form>
        ) : compareWith !== null && compareWith !== shown.number ? (
          <RevisionDiff
            key={`${String(compareWith)}-${String(shown.number)}`}
            specId={view.spec.id}
            artifactId={artifact.id}
            from={compareWith}
            to={shown.number}
          />
        ) : canRender && mode === 'rendered' ? (
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
