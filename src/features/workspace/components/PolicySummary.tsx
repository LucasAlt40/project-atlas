import { useT } from '@/i18n/I18nProvider';
import type { PermissionDto, SecurityPolicyDto } from '@/lib/tauri/commands';
import styles from './Security.module.css';

/** The four areas of a policy in words. Shared by the workspace panel and the agent details. */
export function PolicySummary({
  policy,
  showNetworkNote = false,
}: {
  policy: SecurityPolicyDto;
  /** Spell out that the runtime's own connection to its provider is not governed. */
  showNetworkNote?: boolean;
}) {
  const t = useT();
  const { filesystem, processes, git, network } = policy;
  const destructive: Record<PermissionDto, string | null> = {
    denied: t('security.git.destructiveBlocked'),
    approval_required: t('security.git.destructiveApproval'),
    allowed: t('security.git.destructiveAllowed'),
  };
  return (
    <dl className={styles.areas}>
      <div className={styles.area}>
        <dt>{t('security.filesystem')}</dt>
        <dd>
          <span className={styles.value}>
            <span aria-hidden="true">●</span> {t('security.filesystem.projectOnly')}
          </span>
          <span className={styles.detail}>{t(`security.write.${filesystem.write}`)}</span>
        </dd>
      </div>
      <div className={styles.area}>
        <dt>{t('security.processes')}</dt>
        <dd>
          <span
            className={styles.value}
            title={processes.mode === 'allowed' ? processes.allowedCommands.join(', ') : undefined}
          >
            <span aria-hidden="true">●</span> {t(`security.processes.${processes.mode}`)}
          </span>
          <span className={styles.detail}>{t('security.processes.noShell')}</span>
        </dd>
      </div>
      <div className={styles.area}>
        <dt>{t('security.git')}</dt>
        <dd>
          <span className={styles.value}>
            <span aria-hidden="true">●</span> {gitAccess(t, git.read, git.write)}
          </span>
          <span className={styles.warning}>
            <span aria-hidden="true">⚠</span> {destructive[git.destructive]}
          </span>
        </dd>
      </div>
      <div className={styles.area}>
        <dt>{t('security.network')}</dt>
        <dd>
          <span className={styles.value}>
            <span aria-hidden="true">●</span> {t(`security.network.${network.mode}`)}
          </span>
          {showNetworkNote && <span className={styles.detail}>{t('security.network.note')}</span>}
        </dd>
      </div>
    </dl>
  );
}

function gitAccess(t: ReturnType<typeof useT>, read: PermissionDto, write: PermissionDto): string {
  if (read !== 'allowed') return t('security.git.restricted');
  if (write === 'allowed') return t('security.git.readWrite');
  if (write === 'approval_required') return t('security.git.writeApproval');
  return t('security.git.readOnly');
}
