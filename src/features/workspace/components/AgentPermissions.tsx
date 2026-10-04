import { useState } from 'react';
import type { TranslationKey } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import type { ToolAccessDto } from '@/lib/tauri/commands';
import { useAgentPermissions } from '../hooks/useSecurity';
import { setAgentPermissionProfile } from '../services/workspaceService';
import styles from './Details.module.css';
import { PolicySummary } from './PolicySummary';

const PROFILE_LABEL: Record<string, TranslationKey> = {
  read_only: 'security.profile.read_only',
  developer: 'security.profile.developer',
};

const CAPABILITY_LABEL: Record<string, TranslationKey> = {
  filesystem_write: 'security.capability.filesystem_write',
  process_execution: 'security.capability.process_execution',
  network: 'security.capability.network',
};

/** The tool capabilities a policy can grant, with the runtime's own flag for each. */
const CAPABILITIES = [
  ['filesystem_write', 'filesystemWrite'],
  ['process_execution', 'processExecution'],
  ['network', 'network'],
] as const satisfies readonly (readonly [string, keyof ToolAccessDto])[];

/**
 * What an agent may do in this workspace and the profile that decides it. A new agent has the
 * most restrictive profile (read only), so this is where it is allowed to edit files: the choice
 * is the user's, and the workspace's policy still bounds whatever is picked.
 */
export function AgentPermissions({
  workspaceId,
  agentId,
  runtimeName,
}: {
  workspaceId: string;
  agentId: string;
  runtimeName: string;
}) {
  const { t } = useI18n();
  const { permissions, reload } = useAgentPermissions(workspaceId, agentId);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (permissions.status === 'loading') return <p className={styles.muted}>…</p>;
  if (permissions.status === 'error') {
    return (
      <p role="alert" className={styles.error}>
        {errorMessage(t, permissions.error)}
      </p>
    );
  }
  const { profile, availableProfiles, policy, effective, runtimeAccess, unenforced } =
    permissions.value;

  function change(profileId: string) {
    setSaving(true);
    setError(null);
    setAgentPermissionProfile(agentId, profileId)
      .then(reload)
      .catch((e: unknown) => {
        setError(errorMessage(t, e));
      })
      .finally(() => {
        setSaving(false);
      });
  }

  const labelOf = (labels: Record<string, TranslationKey>, key: string) => {
    const label = labels[key];
    return label ? t(label) : key;
  };
  const list = (keys: string[]) => keys.map((key) => labelOf(CAPABILITY_LABEL, key)).join(', ');
  const policyAllows: Record<(typeof CAPABILITIES)[number][0], boolean> = {
    filesystem_write: policy.filesystem.write !== 'denied',
    process_execution: policy.processes.mode !== 'denied',
    network: policy.network.mode !== 'denied',
  };
  const limited = CAPABILITIES.filter(
    ([key, flag]) => policyAllows[key] && !runtimeAccess[flag],
  ).map(([key]) => key);

  return (
    <>
      <div className={styles.row}>
        <label htmlFor={`profile-${agentId}`}>{t('permissions.profile')}</label>
        <select
          id={`profile-${agentId}`}
          className={styles.select}
          value={profile}
          disabled={saving}
          onChange={(e) => {
            change(e.target.value);
          }}
        >
          {availableProfiles.map((id) => (
            <option key={id} value={id}>
              {labelOf(PROFILE_LABEL, id)}
            </option>
          ))}
        </select>
      </div>
      <PolicySummary policy={effective} />
      {effective.filesystem.write === 'denied' && (
        <p className={styles.note}>{t('permissions.readOnlyHint')}</p>
      )}
      {limited.length > 0 && (
        <p className={styles.note}>
          {t('permissions.runtimeLimits', { runtime: runtimeName, what: list(limited) })}
        </p>
      )}
      {unenforced.length > 0 && (
        <p className={styles.note}>
          {t('permissions.unenforced', { runtime: runtimeName, what: list(unenforced) })}
        </p>
      )}
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
    </>
  );
}
