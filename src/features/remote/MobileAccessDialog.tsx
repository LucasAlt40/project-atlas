import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { useI18n } from '@/i18n/I18nProvider';
import type { RemoteStatusDto } from '@/lib/tauri/commands';
import { newPairingCode, revokeAllDevices, revokeDevice, setRemoteEnabled } from './remoteService';
import { useRemoteAccess } from './useRemoteAccess';
import styles from './Remote.module.css';

function countdown(ms: number): string {
  const total = Math.max(0, Math.ceil(ms / 1000));
  return `${String(Math.floor(total / 60))}:${String(total % 60).padStart(2, '0')}`;
}

function qrSource(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

function CopyButton({ text }: { text: string }) {
  const { t } = useI18n();
  const [copied, setCopied] = useState(false);
  return (
    <Button
      variant="ghost"
      onClick={() => {
        void navigator.clipboard.writeText(text).then(
          () => {
            setCopied(true);
            setTimeout(() => {
              setCopied(false);
            }, 1500);
          },
          () => undefined,
        );
      }}
    >
      {copied ? t('remote.copied') : t('remote.copy')}
    </Button>
  );
}

/**
 * Switches mobile access on and off, shows how to connect a phone and who is connected, and says
 * plainly what it is and is not (ADR 0034). Nothing is on until the person turns it on here.
 */
export function MobileAccessDialog({ onClose }: { onClose: () => void }) {
  const { t, language } = useI18n();
  const { status, adopt } = useRemoteAccess(1000);
  const [address, setAddress] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // Both are the computer's clock, read together, so a skewed system clock cannot mislead.
  const remaining = status?.pairing ? status.pairing.expiresAt - status.serverTime : 0;

  const act = async (work: () => Promise<RemoteStatusDto>) => {
    setBusy(true);
    try {
      adopt(await work());
    } catch {
      // The next read shows where things stand.
    } finally {
      setBusy(false);
    }
  };

  const candidates = status?.candidates ?? [];
  const chosen =
    address ?? candidates.find((c) => c.recommended)?.address ?? candidates[0]?.address ?? null;
  const state = status?.state ?? 'disabled';
  const on = state === 'active';
  const picked = candidates.find((c) => c.address === chosen);

  return (
    <Modal label={t('remote.title')} onClose={onClose}>
      <div className={styles.dialog}>
        <h2 className={styles.title}>{t('remote.title')}</h2>
        <p className={styles.muted}>{t('remote.intro')}</p>

        <div className={styles.stateRow}>
          <span className={styles.state} data-state={state}>
            <span className={styles.dot} data-state={state} aria-hidden="true" />
            {t(`remote.state.${state}`)}
          </span>
          <Button
            variant={on ? 'secondary' : 'primary'}
            disabled={busy || state === 'starting' || (!on && !chosen)}
            onClick={() => {
              void act(() => setRemoteEnabled(!on, on ? null : chosen));
            }}
          >
            {on ? t('remote.turnOff') : t('remote.turnOn')}
          </Button>
        </div>

        {status?.error && (
          <p className={styles.error} role="alert">
            {t(`remote.error.${status.error}`)}
          </p>
        )}

        {!on && (
          <div className={styles.field}>
            <label htmlFor="remote-address" className={styles.label}>
              {t('remote.address')}
            </label>
            {candidates.length === 0 ? (
              <p className={styles.muted}>{t('remote.address.none')}</p>
            ) : (
              <select
                id="remote-address"
                className={styles.select}
                value={chosen ?? ''}
                onChange={(event) => {
                  setAddress(event.target.value);
                }}
              >
                {candidates.map((candidate) => (
                  <option key={candidate.address} value={candidate.address}>
                    {candidate.address} · {candidate.interface} ·{' '}
                    {t(`remote.address.kind.${candidate.kind}`)}
                    {candidate.recommended ? ` · ${t('remote.address.recommended')}` : ''}
                  </option>
                ))}
              </select>
            )}
            <p className={styles.help}>{t('remote.addressHelp')}</p>
            {picked?.kind === 'loopback' && (
              <p className={styles.help}>{t('remote.address.loopbackNote')}</p>
            )}
          </div>
        )}

        {on && status?.addressLost && (
          <p className={styles.warning} role="alert">
            {t('remote.address.lost')}
          </p>
        )}

        {on && status && (
          <>
            {status.waiting > 0 && (
              <p className={styles.attention} role="status">
                {status.waiting === 1
                  ? t('remote.waitingOne')
                  : t('remote.waitingMany', { count: status.waiting })}
              </p>
            )}

            {status.pairing && remaining > 0 ? (
              <div className={styles.pairing}>
                <img
                  className={styles.qr}
                  src={qrSource(status.pairing.qrSvg)}
                  alt={t('remote.qr.alt')}
                  width={224}
                  height={224}
                />
                <div className={styles.pairingText}>
                  <p className={styles.muted}>{t('remote.scan')}</p>
                  <p className={styles.codeLabel}>{t('remote.code')}</p>
                  <p className={styles.code}>{status.pairing.code}</p>
                  <p className={styles.help}>{t('remote.codeHelp', { url: status.url ?? '' })}</p>
                  <p className={styles.help}>
                    {t('remote.expiresIn', { time: countdown(remaining) })}
                  </p>
                </div>
              </div>
            ) : (
              <p className={styles.muted}>{t('remote.noCode')}</p>
            )}
            <div className={styles.row}>
              <Button
                variant="secondary"
                disabled={busy}
                onClick={() => {
                  void act(newPairingCode);
                }}
              >
                {t('remote.newCode')}
              </Button>
              {status.url && (
                <>
                  <code className={styles.url} aria-label={t('remote.url')}>
                    {status.url}
                  </code>
                  <CopyButton text={status.url} />
                </>
              )}
            </div>

            <div className={styles.field}>
              <div className={styles.rowBetween}>
                <h3 className={styles.heading}>{t('remote.devices')}</h3>
                {status.devices.length > 1 && (
                  <Button
                    variant="ghost"
                    disabled={busy}
                    onClick={() => {
                      void act(revokeAllDevices);
                    }}
                  >
                    {t('remote.revokeAll')}
                  </Button>
                )}
              </div>
              {status.devices.length === 0 ? (
                <p className={styles.muted}>{t('remote.devices.none')}</p>
              ) : (
                <ul className={styles.devices}>
                  {status.devices.map((device) => (
                    <li key={device.id} className={styles.device}>
                      <div>
                        <strong>{device.name}</strong>
                        <div className={styles.help}>
                          {t('remote.devices.meta', {
                            address: device.address,
                            time: new Date(device.lastSeen).toLocaleTimeString(language, {
                              hour: '2-digit',
                              minute: '2-digit',
                            }),
                          })}
                        </div>
                      </div>
                      <Button
                        variant="danger"
                        disabled={busy}
                        onClick={() => {
                          void act(() => revokeDevice(device.id));
                        }}
                      >
                        {t('remote.revoke')}
                      </Button>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </>
        )}

        <section className={styles.know} aria-label={t('remote.know.title')}>
          <h3 className={styles.heading}>{t('remote.know.title')}</h3>
          <ul>
            <li>{t('remote.know.computer')}</li>
            <li>{t('remote.know.http')}</li>
            <li>{t('remote.know.can')}</li>
            <li>{t('remote.know.cannot')}</li>
            <li>{t('remote.know.never')}</li>
            <li>{t('remote.know.off')}</li>
          </ul>
        </section>
      </div>
    </Modal>
  );
}
