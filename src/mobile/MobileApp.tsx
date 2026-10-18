import { useCallback, useEffect, useState } from 'react';
import { ErrorBoundary } from '@/components/ui/ErrorBoundary';
import { I18nProvider, useT } from '@/i18n/I18nProvider';
import { isLanguage, type Language } from '@/i18n';
import { AlertBanners, ConnectionBanner } from './Banners';
import { Dashboard } from './Dashboard';
import { codeInAddress, PairScreen } from './PairScreen';
import { RunScreen } from './RunScreen';
import { useRemote, type Remote } from './useRemote';
import styles from './Mobile.module.css';

type Route = { name: 'dashboard' } | { name: 'run'; id: string };

function parseRoute(hash: string): Route {
  const match = /^#\/run\/([^/?#]+)/.exec(hash);
  if (match?.[1]) return { name: 'run', id: decodeURIComponent(match[1]) };
  return { name: 'dashboard' };
}

function useRoute(): Route {
  const [route, setRoute] = useState(() => parseRoute(window.location.hash));
  useEffect(() => {
    const onChange = () => {
      // A pairing code (or anything else) that is not a screen does not stay in the address bar.
      if (window.location.hash.length > 1 && !window.location.hash.startsWith('#/')) {
        window.history.replaceState(null, '', `${window.location.pathname}#/`);
      }
      setRoute(parseRoute(window.location.hash));
      window.scrollTo(0, 0);
    };
    window.addEventListener('hashchange', onChange);
    return () => {
      window.removeEventListener('hashchange', onChange);
    };
  }, []);
  return route;
}

function browserLanguage(): Language {
  return navigator.language.toLowerCase().startsWith('pt') ? 'pt-BR' : 'en-US';
}

function Shell({
  remote,
  linkCode,
  onLinkUsed,
}: {
  remote: Remote;
  linkCode: string | null;
  onLinkUsed: () => void;
}) {
  const t = useT();
  const route = useRoute();
  const needing = remote.runs.filter((run) => run.attention === 'needs_answer').length;

  // The tab says when something waits, even from another tab.
  useEffect(() => {
    document.title = needing > 0 ? `(${String(needing)}) Atlas` : 'Atlas';
  }, [needing]);

  if (remote.phase === 'loading') {
    return (
      <p className={styles.empty} role="status">
        {t('mobile.loading')}
      </p>
    );
  }
  if (remote.phase === 'unpaired') {
    return <PairScreen remote={remote} linkCode={linkCode} onLinkUsed={onLinkUsed} />;
  }
  if (remote.phase === 'unreachable') {
    return (
      <main className={styles.pair}>
        <h1 className={styles.brand}>Atlas</h1>
        <p className={styles.error} role="alert">
          {t('mobile.link.offline')} {t('mobile.link.offlineHelp')}
        </p>
        <button
          type="button"
          className={styles.primary}
          onClick={() => {
            window.location.reload();
          }}
        >
          {t('mobile.retry')}
        </button>
      </main>
    );
  }

  return (
    <div className={styles.app}>
      <header className={styles.top}>
        {route.name === 'run' ? (
          <a className={styles.back} href="#/" aria-label={t('mobile.back')}>
            ‹ {t('mobile.back')}
          </a>
        ) : (
          <span className={styles.brand}>Atlas</span>
        )}
        {remote.deviceName && <span className={styles.device}>{remote.deviceName}</span>}
      </header>
      <ConnectionBanner remote={remote} />
      <AlertBanners
        // What happened to the run being read is already in front of the person.
        alerts={remote.alerts.filter(
          (alert) => !(route.name === 'run' && route.id === alert.runId),
        )}
        onOpen={(alert) => {
          remote.dismissAlert(alert.id);
          window.location.hash = `#/run/${encodeURIComponent(alert.runId)}`;
        }}
        onDismiss={remote.dismissAlert}
      />
      <main className={styles.main}>
        <ErrorBoundary resetKey={route.name === 'run' ? route.id : 'dashboard'}>
          {route.name === 'run' ? (
            <RunScreen key={route.id} remote={remote} id={route.id} />
          ) : (
            <Dashboard remote={remote} />
          )}
        </ErrorBoundary>
      </main>
    </div>
  );
}

/** The phone's page: pair, then follow runs and answer what waits. One screen at a time, no router. */
export function MobileApp() {
  const remote = useRemote();
  // The code in the QR's link is read once, as the page opens (the pairing screen may come later,
  // after the server has said there is no session), and then taken out of the address bar. It is
  // tried once: a code works once, and a pairing screen shown again later (after a disconnect or a
  // revocation) must not send it again, which the computer would count as a wrong code.
  const [linkCode, setLinkCode] = useState(codeInAddress);
  const linkUsed = useCallback(() => {
    setLinkCode(null);
  }, []);
  useEffect(() => {
    if (window.location.hash.length > 1 && !window.location.hash.startsWith('#/')) {
      window.history.replaceState(null, '', `${window.location.pathname}#/`);
    }
  }, []);
  const language: Language =
    remote.language && isLanguage(remote.language) ? remote.language : browserLanguage();
  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);
  return (
    <I18nProvider language={language}>
      <Shell remote={remote} linkCode={linkCode} onLinkUsed={linkUsed} />
    </I18nProvider>
  );
}
