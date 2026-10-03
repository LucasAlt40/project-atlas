import { useState, type SyntheticEvent } from 'react';
import { Button } from '@/components/ui/Button';
import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import type { RuntimesState } from '../hooks/useCatalog';
import type { Agent, CreateAgentInput, Personality } from '../types';
import styles from './Form.module.css';
import { RuntimePicker } from './RuntimePicker';

interface Props {
  personalities: Personality[];
  runtimes: RuntimesState;
  /** Preselected personality for a new agent. */
  initialPersonalityId?: string;
  /** The agent being edited; omit to create a new one. */
  initial?: Agent;
  onRefreshRuntimes: () => void;
  onSubmit: (input: CreateAgentInput) => Promise<Agent>;
  onSaved: (agent: Agent) => void;
  onCancel?: () => void;
}

export function AgentForm({
  personalities,
  runtimes,
  initialPersonalityId = '',
  initial,
  onRefreshRuntimes,
  onSubmit,
  onSaved,
  onCancel,
}: Props) {
  const t = useT();
  const [name, setName] = useState(initial?.name ?? '');
  const [personalityId, setPersonalityId] = useState(
    initial?.personalityId ?? initialPersonalityId,
  );
  const [runtimeId, setRuntimeId] = useState(initial?.runtimeId ?? '');
  const [modelId, setModelId] = useState(initial?.modelId ?? '');
  const [instructions, setInstructions] = useState(initial?.instructions ?? '');
  // On by default: an agent works in an isolated Git worktree unless the user opts out.
  const [worktreeIsolation, setWorktreeIsolation] = useState(initial?.worktreeIsolation ?? true);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const personality = personalities.find((p) => p.id === personalityId);

  function submit(event: SyntheticEvent) {
    event.preventDefault();
    setSaving(true);
    setError(null);
    onSubmit({ name, personalityId, runtimeId, modelId, instructions, worktreeIsolation })
      .then(onSaved)
      .catch((e: unknown) => {
        setError(errorMessage(t, e));
        setSaving(false);
      });
  }

  return (
    <form
      className={styles.form}
      onSubmit={submit}
      aria-label={initial ? t('agents.form.edit', { name: initial.name }) : t('agents.create')}
    >
      <div className={styles.field}>
        <label htmlFor="agent-name" className={styles.label}>
          {t('agents.form.name')}
        </label>
        <input
          id="agent-name"
          className={styles.control}
          value={name}
          placeholder={t('agents.form.namePlaceholder')}
          onChange={(e) => {
            setName(e.target.value);
          }}
        />
      </div>

      <div className={styles.field}>
        <label htmlFor="agent-personality" className={styles.label}>
          {t('agents.form.personality')}
        </label>
        <select
          id="agent-personality"
          className={styles.control}
          value={personalityId}
          onChange={(e) => {
            setPersonalityId(e.target.value);
          }}
        >
          <option value="">{t('agents.form.selectPersonality')}</option>
          {personalities.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
        {personality && <p className={styles.hint}>{personality.description}</p>}
      </div>

      {runtimes.status === 'loading' && <p className={styles.hint}>{t('agents.form.detecting')}</p>}
      {runtimes.status === 'error' && (
        <p role="alert" className={styles.error}>
          {t('agents.form.detectFailed', { message: errorMessage(t, runtimes.error) })}
        </p>
      )}
      {runtimes.status === 'ready' && (
        <RuntimePicker
          runtimes={runtimes.runtimes}
          runtimeId={runtimeId}
          modelId={modelId}
          onRuntimeChange={(id) => {
            setRuntimeId(id);
            setModelId('');
          }}
          onModelChange={setModelId}
        />
      )}
      <div>
        <Button type="button" onClick={onRefreshRuntimes}>
          {t('agents.form.redetect')}
        </Button>
      </div>

      <div className={styles.field}>
        <label htmlFor="agent-instructions" className={styles.label}>
          {t('agents.form.instructions')}
        </label>
        <textarea
          id="agent-instructions"
          className={styles.textarea}
          value={instructions}
          placeholder={t('agents.form.instructionsPlaceholder')}
          onChange={(e) => {
            setInstructions(e.target.value);
          }}
        />
      </div>

      <fieldset className={styles.field}>
        <legend className={styles.label}>{t('agents.form.isolation')}</legend>
        <label className={styles.checkRow} htmlFor="agent-isolation">
          <input
            id="agent-isolation"
            type="checkbox"
            checked={worktreeIsolation}
            onChange={(e) => {
              setWorktreeIsolation(e.target.checked);
            }}
          />
          <span>{t('agents.form.isolationLabel')}</span>
        </label>
        {worktreeIsolation ? (
          <p className={styles.hint}>{t('agents.form.isolationOn')}</p>
        ) : (
          <p className={styles.warning}>{t('agents.form.isolationOff')}</p>
        )}
      </fieldset>

      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
      <div className={styles.actions}>
        {onCancel && (
          <Button type="button" onClick={onCancel}>
            {t('common.cancel')}
          </Button>
        )}
        <Button type="submit" disabled={saving}>
          {initial ? t('agents.form.save') : t('agents.createButton')}
        </Button>
      </div>
    </form>
  );
}
