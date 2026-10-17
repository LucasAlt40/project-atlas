import { useMemo, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import type { Agent } from '@/features/agents/types';
import type { Workspace } from '@/features/workspace/types';
import { useI18n } from '@/i18n/I18nProvider';
import { agentsOf, authorName } from '../model/agents';
import { IntegrationsPicker } from './IntegrationsPicker';
import type { SpecView } from '../types';
import styles from './Specs.module.css';

interface Props {
  workspace: Workspace;
  agents: Agent[];
  view: SpecView;
  busy: boolean;
  onSave: (author: string, reviewer: string, integrations: string[]) => void;
  onClose: () => void;
}

/** Who writes the documents and who reviews them, when that has to change. */
export function AgentsDialog({ workspace, agents, view, busy, onSave, onClose }: Props) {
  const { t } = useI18n();
  const choices = useMemo(() => agentsOf(workspace, agents), [workspace, agents]);
  const [author, setAuthor] = useState(view.spec.authorAgentId);
  const [reviewer, setReviewer] = useState(view.spec.reviewerAgentId);
  const [integrations, setIntegrations] = useState(view.spec.integrations);
  const known = (id: string) => choices.some((agent) => agent.id === id);
  return (
    <Modal label={t('specs.agents.title')} onClose={onClose}>
      <form
        className={styles.form}
        onSubmit={(event) => {
          event.preventDefault();
          onSave(author, reviewer, integrations);
        }}
      >
        <h2 className={styles.dialogTitle}>{t('specs.agents.title')}</h2>
        <p className={styles.muted}>{t('specs.agents.intro')}</p>
        {(['author', 'reviewer'] as const).map((role) => {
          const value = role === 'author' ? author : reviewer;
          const set = role === 'author' ? setAuthor : setReviewer;
          return (
            <label key={role} className={styles.field}>
              <span>{t(`specs.new.${role}`)}</span>
              <select
                value={known(value) ? value : ''}
                onChange={(event) => {
                  set(event.target.value);
                }}
              >
                {!known(value) && <option value="">{t('specs.agents.gone')}</option>}
                {choices.map((agent) => (
                  <option key={agent.id} value={agent.id}>
                    {role === 'author' ? authorName(t, agent) : agent.name}
                  </option>
                ))}
              </select>
            </label>
          );
        })}
        <IntegrationsPicker
          workspaceId={workspace.id}
          selected={integrations}
          onChange={setIntegrations}
        />
        <div className={styles.dialogActions}>
          <Button variant="secondary" onClick={onClose}>
            {t('common.cancel')}
          </Button>
          <Button type="submit" disabled={busy || !known(author) || !known(reviewer)}>
            {t('common.save')}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
