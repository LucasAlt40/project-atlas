import { useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { useI18n } from '@/i18n/I18nProvider';
import type { SpecSummary } from '../types';
import styles from './StartChoice.module.css';

/** How a run begins: from the task alone (vibe coding), or from an approved spec. */
export type StartFrom = { kind: 'vibe' } | { kind: 'spec'; specId: string };

export const VIBE: StartFrom = { kind: 'vibe' };

interface Props {
  specs: SpecSummary[];
  value: StartFrom;
  onChange: (value: StartFrom) => void;
  onNewSpec: () => void;
  /** A run is going or the graph is not runnable: the choice is shown but cannot be changed. */
  disabled?: boolean;
  name: string;
  /** A compact control ("Vibe coding ▾") that opens the choice, for the workflow's run bar. */
  compact?: boolean;
}

/**
 * The question asked before a workflow starts: vibe coding, as it always was, or an approved spec
 * as the source of truth. A spec that is ready and has never been built from is pointed out, but
 * never chosen for the person.
 */
export function StartChoice({
  specs,
  value,
  onChange,
  onNewSpec,
  disabled = false,
  name,
  compact = false,
}: Props) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return undefined;
    const close = (event: MouseEvent) => {
      if (root.current && !root.current.contains(event.target as Node)) setOpen(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', close);
    document.addEventListener('keydown', escape);
    return () => {
      document.removeEventListener('mousedown', close);
      document.removeEventListener('keydown', escape);
    };
  }, [open]);
  const fresh = specs.filter((spec) => spec.implementations === 0);
  const chosen = value.kind === 'spec' ? specs.find((s) => s.id === value.specId) : undefined;
  const choice = (
    <fieldset className={styles.choice} disabled={disabled}>
      <legend>{t('specs.startFrom.title')}</legend>
      {fresh.length > 0 && value.kind === 'vibe' && (
        <p className={styles.fresh} role="status">
          {t('specs.startFrom.newReady', { title: fresh[0]?.title ?? '' })}
        </p>
      )}
      <div className={styles.options}>
        <label className={styles.option} data-selected={value.kind === 'vibe'}>
          <input
            type="radio"
            name={name}
            checked={value.kind === 'vibe'}
            onChange={() => {
              onChange(VIBE);
            }}
          />
          <span>
            <strong>{t('specs.startFrom.vibe')}</strong>
            <span className={styles.help}>{t('specs.startFrom.vibeHelp')}</span>
          </span>
        </label>
        <label className={styles.option} data-selected={value.kind === 'spec'}>
          <input
            type="radio"
            name={name}
            checked={value.kind === 'spec'}
            disabled={specs.length === 0}
            onChange={() => {
              const first = specs.find((s) => s.implementations === 0) ?? specs[0];
              if (first) onChange({ kind: 'spec', specId: first.id });
            }}
          />
          <span>
            <strong>{t('specs.startFrom.spec')}</strong>
            <span className={styles.help}>
              {specs.length === 0 ? t('specs.startFrom.noneReady') : t('specs.startFrom.specHelp')}
            </span>
          </span>
        </label>
      </div>
      {value.kind === 'spec' && specs.length > 0 && (
        <label className={styles.pick}>
          <span>{t('specs.startFrom.choose')}</span>
          <select
            value={chosen?.id ?? ''}
            onChange={(event) => {
              onChange({ kind: 'spec', specId: event.target.value });
            }}
          >
            {chosen === undefined && <option value="">{t('specs.startFrom.gone')}</option>}
            {specs.map((spec) => (
              <option key={spec.id} value={spec.id}>
                {spec.title} ({spec.changeId})
                {spec.implementations > 0
                  ? ` · ${t('specs.implementedBy', { count: spec.implementations })}`
                  : ''}
              </option>
            ))}
          </select>
        </label>
      )}
      <div className={styles.foot}>
        <Button variant="ghost" onClick={onNewSpec}>
          {t('specs.startFrom.write')}
        </Button>
      </div>
    </fieldset>
  );
  if (!compact) return choice;
  return (
    <div className={styles.compact} ref={root}>
      <button
        type="button"
        className={styles.trigger}
        aria-haspopup="dialog"
        aria-expanded={open}
        disabled={disabled}
        aria-label={`${t('specs.startFrom.title')} ${
          value.kind === 'vibe'
            ? t('specs.startFrom.vibe')
            : (chosen?.title ?? t('specs.startFrom.spec'))
        }`}
        title={
          fresh.length > 0 && value.kind === 'vibe'
            ? t('specs.startFrom.newReady', { title: fresh[0]?.title ?? '' })
            : t('specs.startFrom.title')
        }
        onClick={() => {
          setOpen((current) => !current);
        }}
      >
        <Icon name="sparkle" size={15} />
        <span className={styles.triggerLabel}>
          {value.kind === 'vibe'
            ? t('specs.startFrom.vibe')
            : (chosen?.title ?? t('specs.startFrom.spec'))}
        </span>
        {fresh.length > 0 && value.kind === 'vibe' && (
          <span className={styles.freshDot} aria-hidden="true" />
        )}
        <Icon name="chevronDown" size={14} />
      </button>
      {open && <div className={styles.popover}>{choice}</div>}
    </div>
  );
}
