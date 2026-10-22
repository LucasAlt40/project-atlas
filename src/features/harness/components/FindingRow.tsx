import { Badge, type BadgeTone } from '@/components/ui/Badge';
import { useI18n, useT } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type { FindingDto } from '@/lib/tauri/commands';
import { shortPath } from '../model/review';
import { formatAnalyzed } from './StaleNotice';
import harness from './Harness.module.css';
import styles from './Review.module.css';

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
    <span className={harness.tag} data-confidence={finding.confidence}>
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
    <>
      <dl className={styles.meta}>
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
            <dd className={styles.reason}>{finding.reason}</dd>
          </div>
        )}
      </dl>
      <div>
        <h4 className={styles.fieldLabel}>{t('harness.evidence.sources')}</h4>
        <ul className={styles.sources}>
          {finding.evidence.map((e) => (
            <li key={`${e.source}\u0000${e.field ?? ''}`}>
              <code title={e.source}>{shortPath(e.source)}</code>
              {e.field ? <code>{e.field}</code> : null}
            </li>
          ))}
        </ul>
      </div>
    </>
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
  /** Whether the inspector is showing this finding's evidence. */
  selected: boolean;
  onInspect: () => void;
}

const ORIGIN_TONE: Record<string, BadgeTone> = {
  fact: 'success',
  inference: 'running',
  user: 'accent',
};

/**
 * One reviewable finding on one line: include it, correct its value, its origin and confidence,
 * the file it rests on; choosing it shows its evidence beside the list.
 */
export function FindingRow({
  finding,
  included,
  onToggle,
  confirmed,
  onConfirm,
  value,
  onValue,
  selected,
  onInspect,
}: RowProps) {
  const t = useT();
  const name = findingName(t, finding);
  const source = finding.evidence[0]?.source;
  const originKey =
    finding.origin === 'fact' || finding.origin === 'inference' ? finding.origin : 'user';
  return (
    <div className={styles.finding} data-selected={selected} data-excluded={!included}>
      <input
        type="checkbox"
        checked={included}
        aria-label={t('harness.review.include', { label: name })}
        onChange={onToggle}
      />
      <div className={styles.findingValue}>
        {onValue ? (
          <input
            className={styles.valueInput}
            value={value ?? finding.value}
            aria-label={t('harness.review.value', { label: name })}
            onFocus={onInspect}
            onChange={(event) => {
              onValue(event.target.value);
            }}
          />
        ) : (
          <span className={styles.findingName}>{name}</span>
        )}
        {onValue && <span className={styles.findingLabel}>{name}</span>}
      </div>
      <span className={styles.findingMeta}>
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
        <Badge
          dot
          tone={ORIGIN_TONE[originKey] ?? 'neutral'}
          title={t(`harness.origin.${finding.origin}`)}
        >
          {t(`harness.originShort.${originKey}`)}
        </Badge>
        <span className={styles.findingConfidence}>
          {t(`harness.confidenceShort.${finding.confidence}`)}
        </span>
        <button
          type="button"
          className={styles.findingSource}
          aria-pressed={selected}
          aria-label={t('harness.evidence.toggle', { label: name })}
          title={source}
          onClick={onInspect}
        >
          {source === undefined ? t('harness.evidence.button') : shortPath(source)}
        </button>
      </span>
    </div>
  );
}

/** A long statement is named by what comes before its first colon ("Pagbank: PagBank is…"). */
export function inspectorName(name: string): string {
  const colon = name.indexOf(': ');
  return name.length > 60 && colon > 0 && colon < 40 ? name.slice(0, colon) : name;
}
