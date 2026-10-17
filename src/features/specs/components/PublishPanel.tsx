import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { useI18n } from '@/i18n/I18nProvider';
import type { SpecView } from '../types';
import { PanelSection } from './PanelSection';
import styles from './Specs.module.css';

interface Props {
  view: SpecView;
  busy: boolean;
  onPublish: (changeId: string) => void;
  onArchive: () => void;
}

/**
 * Where the spec goes in the project, only once it is ready. Nothing is overwritten: a change
 * folder that exists and is not this spec's makes the person pick another name.
 */
export function PublishPanel({ view, busy, onPublish, onArchive }: Props) {
  const { t } = useI18n();
  const publication = view.spec.publication;
  const [changeId, setChangeId] = useState(view.spec.changeId);
  const ready =
    view.readiness.ready || publication.state === 'failed' || publication.state === 'in_progress';
  const canPublish =
    !view.spec.archived &&
    view.projectAvailable &&
    !view.busy &&
    view.finalApprovalCurrent &&
    view.readiness.blockers.every((b) => b.code.startsWith('publication_'));

  return (
    <PanelSection
      title={t('specs.publish.title')}
      tone={
        publication.state === 'published' && view.publicationCurrent
          ? 'success'
          : publication.state === 'failed'
            ? 'danger'
            : 'neutral'
      }
      defaultOpen={publication.state === 'failed'}
      tag={t(
        publication.state === 'published' && !view.publicationCurrent
          ? 'specs.publish.state.outdated'
          : `specs.publish.state.${publication.state}`,
      )}
    >
      <p className={styles.muted}>{t('specs.publish.intro')}</p>
      {publication.state === 'published' && (
        <p className={styles.mono}>
          {publication.root} · {t('specs.publish.files', { count: publication.files.length })}
        </p>
      )}
      {publication.state === 'failed' && (
        <p className={styles.errorLine} role="alert">
          {t('specs.publish.failed', { done: publication.written.length })} {publication.message}
        </p>
      )}
      {publication.state === 'in_progress' && (
        <p className={styles.warnLine}>{t('specs.publish.leftHalf')}</p>
      )}
      <label className={styles.field}>
        <span>{t('specs.publish.folder')}</span>
        <input
          className={styles.monoInput}
          value={changeId}
          onChange={(event) => {
            setChangeId(event.target.value);
          }}
          aria-describedby="publish-folder-hint"
        />
        <small id="publish-folder-hint" className={styles.hint}>
          {t('specs.publish.folderHint')}
        </small>
      </label>
      <div className={styles.row}>
        <Button
          disabled={!canPublish || !ready || busy || changeId.trim() === ''}
          onClick={() => {
            onPublish(changeId.trim());
          }}
        >
          {publication.state === 'failed' || publication.state === 'in_progress'
            ? t('specs.publish.resume')
            : publication.state === 'published' && !view.publicationCurrent
              ? t('specs.publish.again')
              : t('specs.publish.action')}
        </Button>
        {publication.state === 'published' && view.publicationCurrent && !view.spec.archived && (
          <Button variant="secondary" disabled={busy} onClick={onArchive}>
            {t('specs.publish.archive')}
          </Button>
        )}
      </div>
      {!canPublish && !view.spec.archived && (
        <p className={styles.muted}>
          {view.projectAvailable ? t('specs.publish.needsReady') : t('specs.why.project_missing')}
        </p>
      )}
    </PanelSection>
  );
}
