import { useId } from 'react';
import type { TranslationKey } from '@/i18n';
import { useT } from '@/i18n/I18nProvider';
import {
  CONTRACT_KINDS,
  contractProblem,
  presetContract,
  presetOutcomes,
  type ContractKind,
  type ResultContract,
} from '../model/contract';
import styles from './Form.module.css';

interface Props {
  contract: ResultContract;
  onChange: (contract: ResultContract) => void;
}

/**
 * What the agent promises to say at the end of a step. The workflow only ever sees the outcome
 * ids chosen here: a validator is "pass / fail" because this says so, not because of what it is.
 */
export function ResultContractEditor({ contract, onChange }: Props) {
  const t = useT();
  const id = useId();
  const problem = contractProblem(contract);
  const preset = contract.kind !== 'general' && contract.kind !== 'custom';
  const offered = preset ? presetOutcomes(contract.kind, t) : [];

  const setKind = (kind: ContractKind) => {
    // Custom starts from what is there, so a preset can be turned into an own contract.
    onChange(kind === 'custom' ? { kind, outcomes: contract.outcomes } : presetContract(kind, t));
  };
  const setOutcome = (index: number, patch: Partial<ResultContract['outcomes'][number]>) => {
    onChange({
      ...contract,
      outcomes: contract.outcomes.map((o, i) => (i === index ? { ...o, ...patch } : o)),
    });
  };

  return (
    <fieldset className={[styles.field, styles.group].join(' ')} aria-label={t('contract.title')}>
      <legend className={styles.label}>{t('contract.title')}</legend>
      <label htmlFor={`${id}-kind`} className={styles.hint}>
        {t('contract.type')}
      </label>
      <select
        id={`${id}-kind`}
        className={styles.control}
        value={contract.kind}
        onChange={(e) => {
          setKind(e.target.value as ContractKind);
        }}
      >
        {CONTRACT_KINDS.map((kind) => (
          <option key={kind} value={kind}>
            {t(`contract.kind.${kind}` as TranslationKey)}
          </option>
        ))}
      </select>
      <p className={styles.hint}>{t(`contract.kind.${contract.kind}.hint` as TranslationKey)}</p>

      {preset && (
        <div role="group" aria-label={t('contract.allowed')}>
          {offered.map((outcome) => (
            <label key={outcome.id} className={styles.checkRow}>
              <input
                type="checkbox"
                checked={contract.outcomes.some((o) => o.id === outcome.id)}
                onChange={(e) => {
                  const kept = offered.filter((o) =>
                    o.id === outcome.id
                      ? e.target.checked
                      : contract.outcomes.some((c) => c.id === o.id),
                  );
                  onChange({ ...contract, outcomes: kept });
                }}
              />
              <span>
                <code>{outcome.id}</code> — {outcome.label}
              </span>
            </label>
          ))}
        </div>
      )}

      {contract.kind === 'custom' && (
        <div className={styles.outcomeRows} role="group" aria-label={t('contract.custom')}>
          {contract.outcomes.map((outcome, index) => (
            <div key={index} className={styles.outcomeRow}>
              <input
                className={styles.control}
                aria-label={t('contract.id')}
                placeholder={t('contract.id')}
                value={outcome.id}
                onChange={(e) => {
                  setOutcome(index, { id: e.target.value });
                }}
              />
              <input
                className={styles.control}
                aria-label={t('contract.label')}
                placeholder={t('contract.label')}
                value={outcome.label}
                onChange={(e) => {
                  setOutcome(index, { label: e.target.value });
                }}
              />
              <input
                className={styles.control}
                aria-label={t('contract.description')}
                placeholder={t('contract.description')}
                value={outcome.description ?? ''}
                onChange={(e) => {
                  setOutcome(index, { description: e.target.value });
                }}
              />
              <button
                type="button"
                className={styles.removeButton}
                aria-label={t('contract.remove', { id: outcome.id })}
                onClick={() => {
                  onChange({
                    ...contract,
                    outcomes: contract.outcomes.filter((_, i) => i !== index),
                  });
                }}
              >
                ×
              </button>
            </div>
          ))}
          <div>
            <button
              type="button"
              className={styles.addButton}
              onClick={() => {
                onChange({
                  ...contract,
                  outcomes: [...contract.outcomes, { id: '', label: '', description: '' }],
                });
              }}
            >
              {t('contract.add')}
            </button>
          </div>
        </div>
      )}

      {problem && (
        <p role="alert" className={styles.error}>
          {t(`contract.problem.${problem}` as TranslationKey)}
        </p>
      )}
    </fieldset>
  );
}
