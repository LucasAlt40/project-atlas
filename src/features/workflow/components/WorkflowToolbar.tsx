import { useState } from 'react';
import { useReactFlow, useStore } from '@xyflow/react';
import type { ReactNode } from 'react';
import { Icon, type IconName } from '@/components/ui/Icon';
import type { Agent } from '@/features/agents/types';
import { useI18n } from '@/i18n/I18nProvider';
import styles from './Workflow.module.css';

interface ToolProps {
  icon: IconName;
  disabled?: boolean;
  pressed?: boolean;
  expanded?: boolean;
  label?: string;
  onClick: () => void;
  children?: ReactNode;
}

/** A toolbar action: an icon, and its name unless the icon alone is clear. */
function Tool({ icon, disabled, pressed, expanded, label, onClick, children }: ToolProps) {
  return (
    <button
      type="button"
      className={styles.toolButton}
      disabled={disabled}
      aria-pressed={pressed}
      aria-expanded={expanded}
      aria-label={label}
      title={label}
      onClick={onClick}
    >
      <Icon name={icon} size={15} />
      {children}
    </button>
  );
}

interface NodeToolsProps {
  editable: boolean;
  agents: Agent[];
  hasSelection: boolean;
  canConnect: boolean;
  connecting: boolean;
  onAddAgent: (agent: Agent) => void;
  onAddCondition: () => void;
  onAddEnd: () => void;
  onToggleConnect: () => void;
  onDelete: () => void;
}

/** Adding, connecting and removing nodes: the center of the pipeline bar. */
export function NodeTools({
  editable,
  agents,
  hasSelection,
  canConnect,
  connecting,
  onAddAgent,
  onAddCondition,
  onAddEnd,
  onToggleConnect,
  onDelete,
}: NodeToolsProps) {
  const { t } = useI18n();
  const [choosing, setChoosing] = useState(false);
  return (
    <div className={styles.nodeTools} role="toolbar" aria-label={t('workflow.toolbar')}>
      <div className={styles.toolGroup}>
        <div className={styles.popoverHost}>
          <Tool
            icon="agents"
            disabled={!editable}
            expanded={choosing}
            onClick={() => {
              setChoosing((value) => !value);
            }}
          >
            {t('workflow.toolbar.addAgent')}
          </Tool>
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
        <Tool icon="branch" disabled={!editable} onClick={onAddCondition}>
          {t('workflow.toolbar.addCondition')}
        </Tool>
        <Tool icon="endNode" disabled={!editable} onClick={onAddEnd}>
          {t('workflow.toolbar.addEnd')}
        </Tool>
        <Tool
          icon="link"
          disabled={!editable || !canConnect}
          pressed={connecting}
          onClick={onToggleConnect}
        >
          {t('workflow.toolbar.connect')}
        </Tool>
        <Tool icon="trash" disabled={!editable || !hasSelection} onClick={onDelete}>
          {t('workflow.toolbar.delete')}
        </Tool>
      </div>
    </div>
  );
}

interface CanvasToolsProps {
  editable: boolean;
  canUndo: boolean;
  canRedo: boolean;
  onUndo: () => void;
  onRedo: () => void;
  onResetLayout: () => void;
}

/** History and the view of the canvas: the right end of the pipeline bar. */
export function CanvasTools({
  editable,
  canUndo,
  canRedo,
  onUndo,
  onRedo,
  onResetLayout,
}: CanvasToolsProps) {
  const { t } = useI18n();
  const flow = useReactFlow();
  const zoom = useStore((state) => state.transform[2]);
  return (
    <div className={styles.canvasTools} role="toolbar" aria-label={t('workflow.toolbar.canvas')}>
      <Tool icon="undo" disabled={!editable || !canUndo} onClick={onUndo}>
        {t('workflow.toolbar.undo')}
      </Tool>
      <Tool icon="redo" disabled={!editable || !canRedo} onClick={onRedo}>
        {t('workflow.toolbar.redo')}
      </Tool>
      <Tool icon="layout" disabled={!editable} onClick={onResetLayout}>
        {t('workflow.toolbar.resetLayout')}
      </Tool>
      <Tool
        icon="fit"
        onClick={() => {
          void flow.fitView({ padding: 0.2, maxZoom: 1.1 });
        }}
      >
        {t('workflow.toolbar.fit')}
      </Tool>
      <Tool
        icon="zoomOut"
        label={t('workflow.toolbar.zoomOut')}
        onClick={() => {
          void flow.zoomOut();
        }}
      />
      <span className={styles.zoomValue} aria-label={t('workflow.toolbar.zoomLevel')}>
        {String(Math.round(zoom * 100))}%
      </span>
      <Tool
        icon="zoomIn"
        label={t('workflow.toolbar.zoomIn')}
        onClick={() => {
          void flow.zoomIn();
        }}
      />
    </div>
  );
}
