import { useId, useState } from 'react';
import { useI18n, useT } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type { FindingDto } from '@/lib/tauri/commands';
import { formatAnalyzed } from './StaleNotice';
import styles from './Harness.module.css';

const ARCHITECTURE_KEYS = new Set([
  'layered',
  'clean_architecture',
  'hexagonal',
  'feature_based',
  'monorepo',
  'frontend_backend_split',
]);

/** A finding's name: architecture patterns are concepts and are translated, the rest are names. */
export function findingName(t: ReturnType<typeof useT>, finding: FindingDto): string {
  return finding.category === 'architecture' && ARCHITECTURE_KEYS.has(finding.key)
    ? t(`harness.architecture.${finding.key}` as TranslationKey)
    : finding.label;
}

export function ConfidenceTag({ finding }: { finding: FindingDto }) {
  const t = useT();
  return (
    <span className={styles.tag} data-confidence={finding.confidence}>
      {t(`harness.confidence.${finding.confidence}`)}
    </span>
  );
}

/**
 * The evidence behind a statement: how sure it is, where it comes from (fact, inference, the
 * user) and the files or folders it rests on. This is what lets the user trust, or doubt, it.
 */
export function EvidenceDetails({ finding }: { finding: FindingDto }) {
  const { t, language } = useI18n();
  const { verification } = finding;
  return (
    <dl className={styles.evidence}>
      <div>
        <dt>{t('harness.verification.label')}</dt>
        <dd data-verification={verification.status}>
          {t(`harness.verification.${verification.status}`)}
          {verification.status === 'verified' && verification.method
            ? ` ${t(`harness.verification.method.${verification.method}`)}`
            : ''}
        </dd>
      </div>
      <div>
        <dt>{t('harness.evidence.confidence')}</dt>
        <dd>{t(`harness.confidence.${finding.confidence}`)}</dd>
      </div>
      <div>
        <dt>{t('harness.evidence.origin')}</dt>
        <dd>
          {t(`harness.origin.${finding.origin}`)}
          {finding.byModel ? ` · ${t('harness.evidence.byModel')}` : ''}
        </dd>
      </div>
      {finding.originalOrigin && (
        <div>
          <dt>{t('harness.evidence.originalOrigin')}</dt>
          <dd>{t(`harness.origin.${finding.originalOrigin}`)}</dd>
        </div>
      )}
      {verification.verifiedAt !== undefined && (
        <div>
          <dt>{t('harness.evidence.lastVerified')}</dt>
          <dd>{formatAnalyzed(verification.verifiedAt, language)}</dd>
        </div>
      )}
      {finding.byModel && (
        <div>
          <dt>{t('harness.evidence.value')}</dt>
          <dd className={styles.pre}>{finding.value}</dd>
        </div>
      )}
      {finding.reason && (
        <div>
          <dt>{t('harness.evidence.reason')}</dt>
          <dd>{finding.reason}</dd>
        </div>
      )}
      <div>
        <dt>{t('harness.evidence.sources')}</dt>
        <dd>
          <ul className={styles.sources}>
            {finding.evidence.map((e) => (
              <li key={`${e.source}\u0000${e.field ?? ''}`}>
                <code>{e.source}</code>
                {e.field ? <code className={styles.field}>{e.field}</code> : null}
              </li>
            ))}
          </ul>
        </dd>
      </div>
    </dl>
  );
}

interface RowProps {
  finding: FindingDto;
  included: boolean;
  onToggle: () => void;
  /** An inference can be confirmed by the user. */
  confirmed?: boolean;
  onConfirm?: (() => void) | undefined;
  /** A value the user may correct. */
  value?: string;
  onValue?: ((value: string) => void) | undefined;
}

/** One reviewable finding: include it, correct it, confirm it, look at its evidence. */
export function FindingRow({
  finding,
  included,
  onToggle,
  confirmed,
  onConfirm,
  value,
  onValue,
}: RowProps) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const panelId = useId();
  const name = findingName(t, finding);
  return (
    <div className={styles.findingRow}>
      <div className={styles.row}>
        <label className={styles.check}>
          <input
            type="checkbox"
            checked={included}
            aria-label={t('harness.review.include', { label: name })}
            onChange={onToggle}
          />
          <span>{name}</span>
        </label>
        <span className={styles.rowTools}>
          {finding.origin !== 'fact' && (
            <span className={styles.tag} data-origin={finding.origin}>
              {t(`harness.origin.${finding.origin}`)}
            </span>
          )}
          {finding.confidence !== 'high' && <ConfidenceTag finding={finding} />}
          {onConfirm && (
            <label className={styles.check}>
              <input
                type="checkbox"
                checked={confirmed ?? false}
                aria-label={t('harness.confirm', { label: name })}
                onChange={onConfirm}
              />
              <span>{t('harness.confirmShort')}</span>
            </label>
          )}
          {onValue && (
            <input
              className={styles.value}
              value={value ?? finding.value}
              aria-label={t('harness.review.value', { label: name })}
              onChange={(event) => {
                onValue(event.target.value);
              }}
            />
          )}
          <button
            type="button"
            className={styles.linkButton}
            aria-expanded={open}
            aria-controls={panelId}
            aria-label={t('harness.evidence.toggle', { label: name })}
            onClick={() => {
              setOpen((o) => !o);
            }}
          >
            {t('harness.evidence.button')}
          </button>
        </span>
      </div>
      {open && (
        <div id={panelId} role="region" aria-label={t('harness.evidence.of', { label: name })}>
          <EvidenceDetails finding={finding} />
        </div>
      )}
    </div>
  );
}
