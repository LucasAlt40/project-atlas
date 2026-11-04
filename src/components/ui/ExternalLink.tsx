import { useEffect, useState, type MouseEvent, type ReactNode } from 'react';
import { useT } from '@/i18n/I18nProvider';
import { Icon } from './Icon';

interface Props {
  href: string;
  className?: string | undefined;
  children: ReactNode;
}

/**
 * A link to a page outside Atlas. Atlas has no permission to open a browser, and following it
 * inside the webview would replace Atlas itself: a click copies the address instead (it shows on
 * hover too). Only `https` addresses are shown at all.
 */
export function ExternalLink({ href, className, children }: Props) {
  const t = useT();
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return undefined;
    const timer = setTimeout(() => {
      setCopied(false);
    }, 1500);
    return () => {
      clearTimeout(timer);
    };
  }, [copied]);

  if (!/^https:\/\//i.test(href)) return null;

  const stay = (event: MouseEvent) => {
    event.preventDefault();
  };

  return (
    <a
      href={href}
      title={href}
      className={className}
      onAuxClick={stay}
      onClick={(event) => {
        stay(event);
        // Absent where the page is not a secure context.
        const clipboard = navigator.clipboard as Clipboard | undefined;
        void clipboard?.writeText(href).then(
          () => {
            setCopied(true);
          },
          () => undefined,
        );
      }}
    >
      {copied ? t('link.copied') : children}{' '}
      <Icon name={copied ? 'check' : 'externalLink'} size={12} />
    </a>
  );
}
