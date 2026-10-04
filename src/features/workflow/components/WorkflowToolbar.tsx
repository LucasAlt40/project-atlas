import { useState } from 'react';
import { useReactFlow } from '@xyflow/react';
import { Button } from '@/components/ui/Button';
import type { Agent } from '@/features/agents/types';
import { useI18n } from '@/i18n/I18nProvider';
import styles from './Workflow.module.css';

interface Props {
  editable: boolean;
  agents: Agent[];
  hasSelection: boolean;
  canConnect: boolean;
  connecting: boolean;
  canUndo: boolean;
  canRedo: boolean;
  onAddAgent: (agent: Agent) => void;
  onAddCondition: () => void;
  onAddEnd: () => void;
  onToggleConnect: () => void;
  onDelete: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onResetLayout: () => void;
}

/** What the user can do to the graph. Must sit inside the graph's provider (fit and zoom). */
export function WorkflowToolbar({
  editable,
  agents,
  hasSelection,
  canConnect,
  connecting,
  canUndo,
  canRedo,
  onAddAgent,
  onAddCondition,
  onAddEnd,
  onToggleConnect,
  onDelete,
  onUndo,
  onRedo,
  onResetLayout,
}: Props) {
  const { t } = useI18n();
  const flow = useReactFlow();
  const [choosing, setChoosing] = useState(false);

  return (
    <div className={styles.toolbar} role="toolbar" aria-label={t('workflow.toolbar')}>
      <div className={styles.toolbarGroup}>
        <div className={styles.popoverHost}>
          <Button
            disabled={!editable}
            aria-expanded={choosing}
            onClick={() => {
              setChoosing((value) => !value);
            }}
          >
            {t('workflow.toolbar.addAgent')}
          </Button>
          {choosing && (
            <ul className={styles.popover} aria-label={t('workflow.toolbar.chooseAgent')}>
              {agents.length === 0 && (
                <li className={styles.muted}>{t('workflow.toolbar.noAgents')}</li>
              )}
              {agents.map((agent) => (
                <li key={agent.id}>
                  <button
                    type="button"
                    className={styles.popoverItem}
                    onClick={() => {
                      onAddAgent(agent);
                      setChoosing(false);
                    }}
                  >
                    {agent.name}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
        <Button disabled={!editable} onClick={onAddCondition}>
          {t('workflow.toolbar.addCondition')}
        </Button>
        <Button disabled={!editable} onClick={onAddEnd}>
          {t('workflow.toolbar.addEnd')}
        </Button>
        <Button
          disabled={!editable || !canConnect}
          aria-pressed={connecting}
          onClick={onToggleConnect}
        >
          {t('workflow.toolbar.connect')}
        </Button>
        <Button disabled={!editable || !hasSelection} onClick={onDelete}>
          {t('workflow.toolbar.delete')}
        </Button>
      </div>
      <div className={styles.toolbarGroup}>
        <Button disabled={!editable || !canUndo} onClick={onUndo}>
          {t('workflow.toolbar.undo')}
        </Button>
        <Button disabled={!editable || !canRedo} onClick={onRedo}>
          {t('workflow.toolbar.redo')}
        </Button>
        <Button disabled={!editable} onClick={onResetLayout}>
          {t('workflow.toolbar.resetLayout')}
        </Button>
        <Button
          onClick={() => {
            void flow.fitView({ padding: 0.2, maxZoom: 1.1 });
          }}
        >
          {t('workflow.toolbar.fit')}
        </Button>
        <Button
          aria-label={t('workflow.toolbar.zoomOut')}
          onClick={() => {
            void flow.zoomOut();
          }}
        >
          −
        </Button>
        <Button
          aria-label={t('workflow.toolbar.zoomIn')}
          onClick={() => {
            void flow.zoomIn();
          }}
        >
          +
        </Button>
      </div>
    </div>
  );
}
