import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type { ImportModelAssessmentDto } from '@/lib/tauri/commands';
import styles from './Portability.module.css';

interface Props {
  name: string;
  model: ImportModelAssessmentDto;
  pick: { runtimeId: string; modelId: string } | undefined;
  /** `undefined` keeps the original. */
  onPick: (pick: { runtimeId: string; modelId: string } | undefined) => void;
}

const SEPARATOR = '\u0001';

/**
 * What the agent was exported with, whether this machine can run it, and where else it could run.
 * The original is never swapped for another on its own: the person chooses, or keeps it and sets
 * the agent up later.
 */
export function ModelPanel({ name, model, pick, onPick }: Props) {
  const { t } = useI18n();
  const runtime = model.original.runtimeName ?? model.original.runtimeId;
  const value = pick ? `${pick.runtimeId}${SEPARATOR}${pick.modelId}` : '';
  const needsChoice = !model.keepAllowed;
  return (
    <div className={styles.model} data-status={model.status}>
      <p className={styles.muted}>
        {t('portability.model.original', {
          runtime,
          provider: model.original.provider ?? '—',
          model: model.original.modelId,
        })}
      </p>
      <p className={styles.muted}>
        {t(`portability.model.status.${model.status}` as TranslationKey, {
          runtime,
          model: model.original.modelId,
        })}
      </p>
      {(model.alternatives.length > 0 || needsChoice) && (
        <label className={styles.field}>
          {t('portability.model.choose', { name })}
          <select
            className={styles.control}
            value={value}
            onChange={(e) => {
              const [runtimeId, modelId] = e.target.value.split(SEPARATOR);
              onPick(runtimeId && modelId ? { runtimeId, modelId } : undefined);
            }}
          >
            <option value="">
              {needsChoice ? t('portability.model.pickOne') : t('portability.model.keep')}
            </option>
            {model.alternatives.map((choice) => (
              <optgroup key={choice.runtimeId} label={choice.runtimeName}>
                {choice.models.map((m) => (
                  <option key={m.id} value={`${choice.runtimeId}${SEPARATOR}${m.id}`}>
                    {m.name === m.id ? m.id : `${m.name} (${m.id})`}
                  </option>
                ))}
              </optgroup>
            ))}
          </select>
        </label>
      )}
      {needsChoice && model.alternatives.length === 0 && (
        <p className={styles.error}>{t('portability.model.noAlternative')}</p>
      )}
      {!needsChoice && !pick && model.status !== 'available' && (
        <p className={styles.muted}>{t('portability.model.pending')}</p>
      )}
    </div>
  );
}
