import { useMemo, useState } from 'react';
import { Button, type ButtonVariant } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { useI18n } from '@/i18n/I18nProvider';
import type { ImportReportDto, PortableKindDto } from '@/lib/tauri/commands';
import { ExportDialog } from './ExportDialog';
import { ImportDialog } from './ImportDialog';
import styles from './Portability.module.css';

interface ExportProps {
  kind: PortableKindDto;
  id: string;
  /** Its name, for the dialog and for assistive technology. */
  name: string;
  /** `icon` for a card's action row; `button` for a toolbar. */
  appearance?: 'icon' | 'button';
  variant?: ButtonVariant;
}

/** Exports one configuration: opens the dialog that says what the file will hold. */
export function ExportButton({
  kind,
  id,
  name,
  appearance = 'icon',
  variant = 'secondary',
}: ExportProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const targets = useMemo(() => [{ kind, id }], [kind, id]);
  const label = t('portability.exportItem', { name });
  return (
    <>
      {appearance === 'icon' ? (
        <button
          type="button"
          className={styles.iconButton}
          aria-label={label}
          title={t('portability.export')}
          onClick={() => {
            setOpen(true);
          }}
        >
          <Icon name="upload" size={16} />
        </button>
      ) : (
        <Button
          variant={variant}
          aria-label={label}
          onClick={() => {
            setOpen(true);
          }}
        >
          <Icon name="upload" size={14} /> {t('portability.export')}
        </Button>
      )}
      {open && (
        <ExportDialog
          targets={targets}
          title={t('portability.export.title', { name })}
          onClose={() => {
            setOpen(false);
          }}
        />
      )}
    </>
  );
}

interface ImportProps {
  onImported?: (report: ImportReportDto) => void;
  variant?: ButtonVariant;
}

/** Imports a configuration file: opens the dialog that previews it before anything changes. */
export function ImportButton({ onImported, variant = 'secondary' }: ImportProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button
        variant={variant}
        onClick={() => {
          setOpen(true);
        }}
      >
        <Icon name="download" size={14} /> {t('portability.import')}
      </Button>
      {open && (
        <ImportDialog
          {...(onImported ? { onImported } : {})}
          onClose={() => {
            setOpen(false);
          }}
        />
      )}
    </>
  );
}
