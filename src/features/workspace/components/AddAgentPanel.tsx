import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import type { Agent } from '@/features/agents/types';
import { useT } from '@/i18n/I18nProvider';
import styles from './Workspace.module.css';

interface Props {
  /** Agents that exist but are not in this workspace. */
  available: Agent[];
  onAdd: (agentId: string) => void;
  onCreateNew: () => void;
  onClose: () => void;
}

/** Adding an agent: one of those already created, or a new one (the Agents form). */
export function AddAgentPanel({ available, onAdd, onCreateNew, onClose }: Props) {
  const t = useT();
  return (
    <Modal
      label={t('workspace.addPanel.title')}
      title={t('workspace.addPanel.title')}
      size="sm"
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t('common.cancel')}
          </Button>
          <Button onClick={onCreateNew}>{t('workspace.addPanel.createNew')}</Button>
        </>
      }
    >
      {available.length > 0 && (
        <ul className={styles.available}>
          {available.map((agent) => (
            <li key={agent.id}>
              <span className={styles.availableName}>{agent.name}</span>
              <span className={styles.availableModel}>{agent.modelId}</span>
              <Button
                variant="secondary"
                size="sm"
                onClick={() => {
                  onAdd(agent.id);
                }}
              >
                {t('workspace.addPanel.add', { name: agent.name })}
              </Button>
            </li>
          ))}
        </ul>
      )}
    </Modal>
  );
}
