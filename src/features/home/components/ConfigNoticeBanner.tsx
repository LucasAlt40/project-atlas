import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Callout } from '@/components/ui/Callout';
import { useT } from '@/i18n/I18nProvider';
import { useAppInfo } from '../hooks/useAppInfo';

/**
 * Says, once per start, that the stored config could not be read as it was and where the
 * original file is: Atlas never discards a person's data without telling them.
 */
export function ConfigNoticeBanner() {
  const t = useT();
  const state = useAppInfo();
  const [dismissed, setDismissed] = useState(false);
  if (dismissed || state.status !== 'ready' || !state.info.configNotice) return null;
  const notice = state.info.configNotice;

  const dismiss = (
    <Button
      variant="ghost"
      onClick={() => {
        setDismissed(true);
      }}
    >
      {t('configNotice.dismiss')}
    </Button>
  );

  switch (notice.kind) {
    case 'recovered':
      return (
        <Callout
          tone="warning"
          role="alert"
          title={t('configNotice.recovered.title')}
          action={dismiss}
        >
          {t('configNotice.recovered.body', {
            sections: notice.dropped.map((d) => `${d.section}: ${String(d.count)}`).join(', '),
            backup: notice.backup,
          })}
        </Callout>
      );
    case 'reset':
      return (
        <Callout tone="danger" role="alert" title={t('configNotice.reset.title')} action={dismiss}>
          {t('configNotice.reset.body', { backup: notice.backup })}
        </Callout>
      );
    case 'unreadable':
      return (
        <Callout
          tone="danger"
          role="alert"
          title={t('configNotice.unreadable.title')}
          action={dismiss}
        >
          {t('configNotice.unreadable.body', { path: notice.path })}
        </Callout>
      );
  }
}
