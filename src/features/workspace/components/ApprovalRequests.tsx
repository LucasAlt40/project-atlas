import { Button } from '@/components/ui/Button';
import { useT } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type { ApprovalRequest } from '../model/approvals';
import styles from './Security.module.css';

interface Props {
  requests: ApprovalRequest[];
  agentName: (agentId: string) => string;
  onResolve: (id: string, approve: boolean) => void;
}

/**
 * Commands an agent wants to run that Atlas will not run on its own. The core has paused that
 * execution; these buttons are the only thing that releases it. Chat text can never answer.
 */
export function ApprovalRequests({ requests, agentName, onResolve }: Props) {
  const t = useT();
  if (requests.length === 0) return null;
  return (
    <section className={styles.approvals} aria-label={t('approval.title')} aria-live="assertive">
      {requests.map((request) => (
        <article key={request.id} className={styles.approval} aria-label={t('approval.request')}>
          <h2 className={styles.approvalTitle}>
            <span aria-hidden="true">⚠</span> {t('approval.title')}
          </h2>
          <dl className={styles.approvalFacts}>
            <div>
              <dt>{t('approval.agent')}</dt>
              <dd>{agentName(request.agentId)}</dd>
            </div>
            <div>
              <dt>{t('approval.wants')}</dt>
              <dd>
                {t('approval.runCommand')}
                <code className={styles.command}>{request.command}</code>
              </dd>
            </div>
            {request.cwd && (
              <div>
                <dt>{t('approval.directory')}</dt>
                <dd>{request.cwd}</dd>
              </div>
            )}
            {request.reason && (
              <div>
                <dt>{t('approval.reason')}</dt>
                <dd>{t(`permission.reason.${request.reason}` as TranslationKey)}</dd>
              </div>
            )}
          </dl>
          <div className={styles.approvalActions}>
            <Button
              onClick={() => {
                onResolve(request.id, true);
              }}
            >
              {t('approval.allowOnce')}
            </Button>
            <Button
              variant="secondary"
              onClick={() => {
                onResolve(request.id, false);
              }}
            >
              {t('approval.deny')}
            </Button>
          </div>
        </article>
      ))}
    </section>
  );
}
