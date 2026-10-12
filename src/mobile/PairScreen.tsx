import { useEffect, useRef, useState, type SyntheticEvent } from 'react';
import { useT } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import { ApiError } from './api';
import { deviceNameFromAgent, pairingCodeFrom } from './format';
import type { Remote } from './useRemote';
import styles from './Mobile.module.css';

const CODE_LENGTH = 16;

function errorKey(error: unknown): TranslationKey {
  if (error instanceof ApiError) {
    switch (error.code) {
      case 'invalid_code':
        return 'mobile.pair.invalid';
      case 'pairing_locked':
        return 'mobile.pair.locked';
      case 'too_many_devices':
        return 'mobile.pair.tooManyDevices';
      case 'rate_limited':
        return 'mobile.pair.rateLimited';
      case 'unreachable':
        return 'mobile.pair.unreachable';
      default:
        return 'mobile.pair.failed';
    }
  }
  return 'mobile.pair.failed';
}

/**
 * The first screen of a phone that has no session. The code arrives in the address fragment (the
 * QR on the computer's screen), which the browser never sends to the server; it is removed from
 * the address bar before anything else happens, so it does not stay in the history.
 */
/** A pairing code carried by the address (the QR's link), if there is one. */
export function codeInAddress(): string | null {
  const hash = window.location.hash;
  if (hash.length <= 1 || hash.startsWith('#/')) return null;
  const code = pairingCodeFrom(hash.slice(1));
  return code.length === CODE_LENGTH ? code : null;
}

export function PairScreen({ remote, linkCode }: { remote: Remote; linkCode: string | null }) {
  const t = useT();
  const [typed, setTyped] = useState('');
  const [error, setError] = useState<TranslationKey | null>(null);
  const [wait, setWait] = useState<number | null>(null);
  const [busy, setBusy] = useState(linkCode !== null);
  const attempted = useRef(false);

  const attempt = (code: string) => {
    remote
      .pair(code, deviceNameFromAgent(navigator.userAgent))
      .then(() => {
        window.history.replaceState(null, '', `${window.location.pathname}#/`);
      })
      .catch((failure: unknown) => {
        setError(errorKey(failure));
        setWait(
          failure instanceof ApiError && failure.retryAfterMs
            ? Math.ceil(failure.retryAfterMs / 1000)
            : null,
        );
        setBusy(false);
      });
  };

  useEffect(() => {
    if (attempted.current) return;
    attempted.current = true;
    if (linkCode) attempt(linkCode);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- once, when the page opens
  }, []);

  const submit = (event: SyntheticEvent) => {
    event.preventDefault();
    const code = pairingCodeFrom(typed);
    if (code.length !== CODE_LENGTH) {
      setError('mobile.pair.invalid');
      return;
    }
    setBusy(true);
    setError(null);
    attempt(code);
  };

  const reason =
    remote.unpairedReason === 'revoked'
      ? t('mobile.pair.revoked')
      : remote.unpairedReason === 'expired'
        ? t('mobile.pair.expired')
        : null;

  return (
    <main className={styles.pair}>
      <h1 className={styles.brand}>Atlas</h1>
      <p className={styles.lead}>{t('mobile.pair.lead')}</p>
      {reason && (
        <p className={styles.notice} role="status">
          {reason}
        </p>
      )}
      <ol className={styles.steps}>
        <li>{t('mobile.pair.step1')}</li>
        <li>{t('mobile.pair.step2')}</li>
      </ol>
      <form onSubmit={submit} className={styles.form}>
        <label htmlFor="pair-code" className={styles.label}>
          {t('mobile.pair.codeLabel')}
        </label>
        <input
          id="pair-code"
          className={styles.input}
          value={typed}
          onChange={(event) => {
            setTyped(event.target.value);
          }}
          autoComplete="off"
          autoCapitalize="characters"
          autoCorrect="off"
          spellCheck={false}
          inputMode="text"
          placeholder="ABCD-EFGH-IJKL-MNOP"
          aria-describedby={error ? 'pair-error' : undefined}
          aria-invalid={error ? true : undefined}
        />
        <button type="submit" className={styles.primary} disabled={busy}>
          {busy ? t('mobile.pair.connecting') : t('mobile.pair.connect')}
        </button>
      </form>
      {error && (
        <p id="pair-error" className={styles.error} role="alert">
          {t(error, { seconds: wait ?? 0 })}
        </p>
      )}
      <p className={styles.fine}>{t('mobile.pair.fine')}</p>
    </main>
  );
}
