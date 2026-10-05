import { Button } from '@/components/ui/Button';
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

export function AddAgentPanel({ available, onAdd, onCreateNew, onClose }: Props) {
  const t = useT();
  return (
    <section className={styles.panel} aria-label={t('workspace.addPanel.title')}>
      <h2 className={styles.panelTitle}>{t('workspace.addPanel.title')}</h2>
      {available.length > 0 && (
        <ul className={styles.available}>
          {available.map((agent) => (
            <li key={agent.id}>
              <span>{agent.name}</span>
              <Button
                variant="secondary"
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
      <div className={styles.panelActions}>
        <Button onClick={onCreateNew}>{t('workspace.addPanel.createNew')}</Button>
        <button type="button" className={styles.linkButton} onClick={onClose}>
          {t('common.cancel')}
        </button>
      </div>
    </section>
  );
}
