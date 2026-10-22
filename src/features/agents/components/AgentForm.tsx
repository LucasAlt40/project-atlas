import { useState, type SyntheticEvent } from 'react';
import { ActionBar } from '@/components/ui/ActionBar';
import { Button } from '@/components/ui/Button';
import { Callout } from '@/components/ui/Callout';
import { Icon } from '@/components/ui/Icon';
import type { TranslationKey } from '@/i18n';
import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import type { RuntimesState } from '../hooks/useCatalog';
import { contractProblem, GENERAL_CONTRACT, type ResultContract } from '../model/contract';
import type { Agent, CreateAgentInput, Personality } from '../types';
import styles from './Form.module.css';
import { ResultContractEditor } from './ResultContractEditor';
import { RuntimePicker } from './RuntimePicker';

const PROFILES: { id: string; label: TranslationKey; hint: TranslationKey }[] = [
  {
    id: 'read_only',
    label: 'security.profile.read_only',
    hint: 'agents.form.permissionReadOnlyHint',
  },
  {
    id: 'developer',
    label: 'security.profile.developer',
    hint: 'agents.form.permissionDeveloperHint',
  },
];

/** What a personality suggests for a new agent; the editor starts from it, the user decides. */
/** The ids of a contract's outcomes, in a form that compares regardless of the label language. */
const outcomeIds = (contract: ResultContract) =>
  contract.outcomes
    .map((outcome) => outcome.id)
    .sort()
    .join(',');

const FALLBACK_PROFILE = 'developer';

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
  /** `page`: the agents screen, with a footer fixed at the bottom; `inline`: inside a panel. */
  variant?: 'page' | 'inline';
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
  variant = 'page',
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
  // A new agent starts from what its personality suggests, until the user chooses otherwise;
  // an existing one keeps its own.
  const suggestion = (id: string): ResultContract =>
    personalities.find((p) => p.id === id)?.suggestedContract ?? GENERAL_CONTRACT;
  const [contract, setContract] = useState<ResultContract>(
    initial?.resultContract ?? suggestion(initialPersonalityId),
  );
  const [contractTouched, setContractTouched] = useState(initial !== undefined);
  const profileSuggestion = (id: string) =>
    personalities.find((p) => p.id === id)?.suggestedPermissionProfile ?? FALLBACK_PROFILE;
  // A new agent starts from what its personality suggests, until the user chooses; an existing
  // one keeps its own (an agent saved without a profile reads as read only).
  const [profile, setProfile] = useState(
    initial
      ? (initial.permissionProfileId ?? 'read_only')
      : profileSuggestion(initialPersonalityId),
  );
  const [profileTouched, setProfileTouched] = useState(initial !== undefined);
  // Whether the user picked a profile in this form: an edit that did not leaves the agent's own.
  const [profileChanged, setProfileChanged] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const personality = personalities.find((p) => p.id === personalityId);
  const runtime =
    runtimes.status === 'ready' ? runtimes.runtimes.find((r) => r.runtime.id === runtimeId) : null;

  function submit(event: SyntheticEvent) {
    event.preventDefault();
    if (contractProblem(contract)) return;
    setSaving(true);
    setError(null);
    onSubmit({
      name,
      personalityId,
      runtimeId,
      modelId,
      instructions,
      worktreeIsolation,
      resultContract: contract,
      // An edit that does not touch it leaves the agent's profile as it is.
      ...(initial && !profileChanged ? {} : { permissionProfileId: profile }),
    })
      .then(onSaved)
      .catch((e: unknown) => {
        setError(errorMessage(t, e));
        setSaving(false);
      });
  }

  const suggested = personality
    ? { contract: suggestion(personality.id), profile: profileSuggestion(personality.id) }
    : null;
  const followsSuggestion =
    suggested !== null &&
    suggested.profile === profile &&
    suggested.contract.kind === contract.kind &&
    outcomeIds(suggested.contract) === outcomeIds(contract);
  const profileLabel = (id: string) =>
    t(PROFILES.find((option) => option.id === id)?.label ?? 'security.profile.developer');
  const sectionId = (name: string) => `agent-form-${name}`;

  return (
    <form
      className={styles.agentForm}
      data-variant={variant}
      onSubmit={submit}
      aria-label={initial ? t('agents.form.edit', { name: initial.name }) : t('agents.create')}
    >
      <section className={styles.section}>
        <h2 id={sectionId('identity')} className={styles.sectionTitle}>
          {t('agents.form.section.identity')}
        </h2>
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
              if (!contractTouched) setContract(suggestion(e.target.value));
              if (!profileTouched) setProfile(profileSuggestion(e.target.value));
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
        {suggested && (
          <Callout
            tone="info"
            action={
              followsSuggestion ? undefined : (
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={() => {
                    setContract(suggested.contract);
                    setContractTouched(true);
                    setProfile(suggested.profile);
                    setProfileTouched(true);
                    setProfileChanged(true);
                  }}
                >
                  {t('agents.form.applySuggestion')}
                </Button>
              )
            }
          >
            {t('agents.form.suggestion', {
              contract: t(`contract.kind.${suggested.contract.kind}` as TranslationKey),
              profile: profileLabel(suggested.profile),
            })}
          </Callout>
        )}
      </section>

      <section className={styles.section}>
        <h2 id={sectionId('runtime')} className={styles.sectionTitle}>
          {t('agents.form.section.runtime')}
        </h2>
        {runtimes.status === 'loading' && (
          <p className={styles.hint}>{t('agents.form.detecting')}</p>
        )}
        {runtimes.status === 'error' && (
          <Callout tone="danger" role="alert">
            {t('agents.form.detectFailed', { message: errorMessage(t, runtimes.error) })}
          </Callout>
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
        {runtime && !runtime.runtime.capabilities.fileEdit && (
          <p className={styles.hint}>{t('agents.form.noFileEdit')}</p>
        )}
        <div>
          <Button type="button" variant="ghost" size="sm" onClick={onRefreshRuntimes}>
            <Icon name="refresh" size={14} />
            {t('agents.form.redetect')}
          </Button>
        </div>
      </section>

      <section className={styles.section}>
        <h2 id={sectionId('instructions')} className={styles.sectionTitle}>
          {t('agents.form.section.instructions')}
        </h2>
        <textarea
          id="agent-instructions"
          className={styles.textarea}
          rows={3}
          aria-labelledby={sectionId('instructions')}
          value={instructions}
          placeholder={t('agents.form.instructionsPlaceholder')}
          onChange={(e) => {
            setInstructions(e.target.value);
          }}
        />
      </section>

      <section className={styles.section}>
        <h2 id={sectionId('contract')} className={styles.sectionTitle}>
          {t('agents.form.section.contract')}
        </h2>
        <ResultContractEditor
          contract={contract}
          onChange={(next) => {
            setContractTouched(true);
            setContract(next);
          }}
        />
      </section>

      <section className={styles.section}>
        <h2 id={sectionId('security')} className={styles.sectionTitle}>
          {t('agents.form.section.security')}
        </h2>
        <div className={styles.choices} role="radiogroup" aria-label={t('agents.form.permissions')}>
          {PROFILES.map((option) => (
            <label key={option.id} className={styles.choice}>
              <input
                type="radio"
                name="agent-permissions"
                value={option.id}
                checked={profile === option.id}
                aria-labelledby={`agent-permission-${option.id}-title`}
                aria-describedby={`agent-permission-${option.id}`}
                onChange={() => {
                  setProfileTouched(true);
                  setProfileChanged(true);
                  setProfile(option.id);
                }}
              />
              <span className={styles.choiceText}>
                <span id={`agent-permission-${option.id}-title`} className={styles.choiceTitle}>
                  {t(option.label)}
                </span>
                <span id={`agent-permission-${option.id}`} className={styles.hint}>
                  {t(option.hint)}
                </span>
              </span>
            </label>
          ))}
        </div>

        <div className={styles.isolation}>
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
        </div>
      </section>

      {error && (
        <Callout tone="danger" role="alert">
          {error}
        </Callout>
      )}
      <div className={styles.footer}>
        <ActionBar
          variant={variant === 'page' ? 'page' : 'inline'}
          end={
            <>
              {onCancel && (
                <Button type="button" variant="secondary" onClick={onCancel}>
                  {t('common.cancel')}
                </Button>
              )}
              <Button type="submit" disabled={saving}>
                {initial ? t('agents.form.save') : t('agents.createButton')}
              </Button>
            </>
          }
        />
      </div>
    </form>
  );
}
