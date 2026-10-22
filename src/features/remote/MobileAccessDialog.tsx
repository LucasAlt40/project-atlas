import { useState } from 'react';
import { Badge, type BadgeTone } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { Callout } from '@/components/ui/Callout';
import { CollapsibleSection } from '@/components/ui/CollapsibleSection';
import { Icon } from '@/components/ui/Icon';
import { Modal } from '@/components/ui/Modal';
import { useI18n } from '@/i18n/I18nProvider';
import type { RemoteStatusDto } from '@/lib/tauri/commands';
import { newPairingCode, revokeAllDevices, revokeDevice, setRemoteEnabled } from './remoteService';
import { useRemoteAccess } from './useRemoteAccess';
import styles from './Remote.module.css';

const STATE_TONE: Record<RemoteStatusDto['state'], BadgeTone> = {
  disabled: 'neutral',
  starting: 'running',
  active: 'success',
  error: 'danger',
};

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
    <Modal
      label={t('remote.title')}
      onClose={onClose}
      size="md"
      title={t('remote.title')}
      footer={
        <Button variant="secondary" onClick={onClose}>
          {t('common.close')}
        </Button>
      }
    >
      <div className={styles.dialog}>
        <p className={styles.muted}>{t('remote.intro')}</p>

        <div className={styles.stateRow}>
          <Badge dot tone={STATE_TONE[state]}>
            {t(`remote.state.${state}`)}
          </Badge>
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
          <Callout tone="danger" role="alert">
            {t(`remote.error.${status.error}`)}
          </Callout>
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
          <Callout tone="warning" role="alert">
            {t('remote.address.lost')}
          </Callout>
        )}

        {on && status && (
          <>
            {status.waiting > 0 && (
              <Callout tone="warning" role="status">
                {status.waiting === 1
                  ? t('remote.waitingOne')
                  : t('remote.waitingMany', { count: status.waiting })}
              </Callout>
            )}

            {status.pairing && remaining > 0 ? (
              <div className={styles.pairing}>
                <img
                  className={styles.qr}
                  src={qrSource(status.pairing.qrSvg)}
                  alt={t('remote.qr.alt')}
                  width={160}
                  height={160}
                />
                <div className={styles.pairingText}>
                  <p className={styles.muted}>{t('remote.scan')}</p>
                  <p className={styles.codeLabel}>{t('remote.code')}</p>
                  <p className={styles.code}>{status.pairing.code}</p>
                  <div className={styles.rowBetween}>
                    <span className={styles.help}>
                      {t('remote.expiresIn', { time: countdown(remaining) })}
                    </span>
                    <Button
                      variant="ghost"
                      size="sm"
                      disabled={busy}
                      onClick={() => {
                        void act(newPairingCode);
                      }}
                    >
                      <Icon name="refresh" size={14} />
                      {t('remote.newCode')}
                    </Button>
                  </div>
                  <p className={styles.help}>{t('remote.codeHelp', { url: status.url ?? '' })}</p>
                  {status.url && (
                    <div className={styles.urlRow}>
                      <code className={styles.url} aria-label={t('remote.url')}>
                        {status.url}
                      </code>
                      <CopyButton text={status.url} />
                    </div>
                  )}
                </div>
              </div>
            ) : (
              <div className={styles.stack}>
                <p className={styles.muted}>{t('remote.noCode')}</p>
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
              </div>
            )}

            <div className={styles.field}>
              <div className={styles.rowBetween}>
                <h3 className={styles.heading}>
                  {t('remote.devices')}
                  {status.devices.length > 0 ? ` (${String(status.devices.length)})` : ''}
                </h3>
                {status.devices.length > 1 && (
                  <Button
                    variant="ghost"
                    size="sm"
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
                        variant="secondary"
                        size="sm"
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

        <CollapsibleSection title={t('remote.know.title')} defaultOpen={false}>
          <ul className={styles.knowList}>
            <li>{t('remote.know.computer')}</li>
            <li>{t('remote.know.http')}</li>
            <li>{t('remote.know.can')}</li>
            <li>{t('remote.know.cannot')}</li>
            <li>{t('remote.know.never')}</li>
            <li>{t('remote.know.off')}</li>
          </ul>
        </CollapsibleSection>
      </div>
    </Modal>
  );
}
