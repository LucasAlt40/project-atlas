import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Callout } from '@/components/ui/Callout';
import { useI18n } from '@/i18n/I18nProvider';
import type { HandEditDto } from '@/lib/tauri/commands';
import { stageName } from '../model/messages';
import { specHandEdits } from '../services/specService';

interface Props {
  specId: string;
  /** Read again whenever the spec changes. */
  version: number;
  busy: boolean;
  onAdopt: (artifactId: string) => void;
}

/**
 * The published files that were edited by hand in the project. Atlas never writes over them: the
 * person adopts an edit (it becomes the document's next revision) or undoes it in the project.
 */
export function HandEditsNotice({ specId, version, busy, onAdopt }: Props) {
  const { t } = useI18n();
  const [edits, setEdits] = useState<HandEditDto[]>([]);

  useEffect(() => {
    let current = true;
    specHandEdits(specId)
      .then((found) => {
        if (current) setEdits(found);
      })
      .catch(() => {
        if (current) setEdits([]);
      });
    return () => {
      current = false;
    };
  }, [specId, version]);

  return (
    <>
      {edits.map((edit) => (
        <Callout
          key={edit.artifactId}
          tone="warning"
          title={t('specs.handEdits.title', { stage: stageName(t, edit.artifactId) })}
          action={
            edit.missing.length === 0 ? (
              <Button
                size="sm"
                variant="secondary"
                disabled={busy}
                onClick={() => {
                  onAdopt(edit.artifactId);
                }}
              >
                {t('specs.handEdits.adopt')}
              </Button>
            ) : undefined
          }
        >
          {edit.changed.length > 0 &&
            t('specs.handEdits.changed', { files: edit.changed.join(', ') })}{' '}
          {edit.missing.length > 0 &&
            t('specs.handEdits.missing', { files: edit.missing.join(', ') })}{' '}
          {t('specs.handEdits.never')}
        </Callout>
      ))}
    </>
  );
}
