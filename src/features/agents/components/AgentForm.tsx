import { useState, type SyntheticEvent } from 'react';
import { Button } from '@/components/ui/Button';
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
    id: 'developer',
    label: 'security.profile.developer',
    hint: 'agents.form.permissionDeveloperHint',
  },
  {
    id: 'read_only',
    label: 'security.profile.read_only',
    hint: 'agents.form.permissionReadOnlyHint',
  },
];

/** What a personality suggests for a new agent; the editor starts from it, the user decides. */
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

  return (
    <form
      className={styles.agentForm}
      onSubmit={submit}
      aria-label={initial ? t('agents.form.edit', { name: initial.name }) : t('agents.create')}
    >
      <div className={styles.columns}>
        <div className={styles.column}>
          <section className={styles.section}>
            <h2 className={styles.sectionTitle}>{t('agents.form.section.identity')}</h2>
            <div className={styles.pair}>
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
            </div>
          </section>

          <section className={styles.section}>
            <h2 className={styles.sectionTitle}>{t('agents.form.section.runtime')}</h2>
            {runtimes.status === 'loading' && (
              <p className={styles.hint}>{t('agents.form.detecting')}</p>
            )}
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
            {runtime && !runtime.runtime.capabilities.fileEdit && (
              <p className={styles.hint}>{t('agents.form.noFileEdit')}</p>
            )}
            <div>
              <Button type="button" onClick={onRefreshRuntimes}>
                {t('agents.form.redetect')}
              </Button>
            </div>
          </section>

          <section className={styles.section}>
            <h2 className={styles.sectionTitle}>{t('agents.form.section.instructions')}</h2>
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
          </section>
        </div>

        <div className={styles.column}>
          <section className={styles.section}>
            <h2 className={styles.sectionTitle}>{t('agents.form.section.contract')}</h2>
            <ResultContractEditor
              contract={contract}
              onChange={(next) => {
                setContractTouched(true);
                setContract(next);
              }}
            />
          </section>

          <section className={styles.section}>
            <h2 className={styles.sectionTitle}>{t('agents.form.section.security')}</h2>
            <div className={styles.field}>
              <label htmlFor="agent-permissions" className={styles.label}>
                {t('agents.form.permissions')}
              </label>
              <select
                id="agent-permissions"
                className={styles.control}
                value={profile}
                onChange={(e) => {
                  setProfileTouched(true);
                  setProfileChanged(true);
                  setProfile(e.target.value);
                }}
              >
                {PROFILES.map((option) => (
                  <option key={option.id} value={option.id}>
                    {t(option.label)}
                  </option>
                ))}
              </select>
              <p className={styles.hint}>
                {t(
                  PROFILES.find((option) => option.id === profile)?.hint ??
                    'agents.form.permissionDeveloperHint',
                )}
              </p>
              {!initial && !profileTouched && personality && (
                <p className={styles.hint}>
                  {t('agents.form.permissionSuggested', { personality: personality.name })}
                </p>
              )}
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
          </section>
        </div>
      </div>

      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
      <div className={styles.stickyBar}>
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
