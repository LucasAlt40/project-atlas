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
            icon="plus"
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
        <Tool icon="plus" disabled={!editable} onClick={onAddCondition}>
          {t('workflow.toolbar.addCondition')}
        </Tool>
        <Tool icon="plus" disabled={!editable} onClick={onAddEnd}>
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

interface HistoryToolsProps {
  editable: boolean;
  canUndo: boolean;
  canRedo: boolean;
  onUndo: () => void;
  onRedo: () => void;
  onResetLayout: () => void;
}

/** Undo, redo and the automatic layout: the end of the floating toolbar. */
export function HistoryTools({
  editable,
  canUndo,
  canRedo,
  onUndo,
  onRedo,
  onResetLayout,
}: HistoryToolsProps) {
  const { t } = useI18n();
  return (
    <div className={styles.toolGroup} role="group" aria-label={t('workflow.toolbar.canvas')}>
      <Tool
        icon="undo"
        label={t('workflow.toolbar.undo')}
        disabled={!editable || !canUndo}
        onClick={onUndo}
      />
      <Tool
        icon="redo"
        label={t('workflow.toolbar.redo')}
        disabled={!editable || !canRedo}
        onClick={onRedo}
      />
      <Tool icon="layout" disabled={!editable} onClick={onResetLayout}>
        {t('workflow.toolbar.resetLayout')}
      </Tool>
    </div>
  );
}

/** The view of the canvas: zoom and fit, at its lower right. */
export function ZoomTools() {
  const { t } = useI18n();
  const flow = useReactFlow();
  const zoom = useStore((state) => state.transform[2]);
  return (
    <div className={styles.zoomTools} role="toolbar" aria-label={t('workflow.toolbar.zoom')}>
      <Tool
        icon="minus"
        label={t('workflow.toolbar.zoomOut')}
        onClick={() => {
          void flow.zoomOut();
        }}
      />
      <span className={styles.zoomValue} aria-label={t('workflow.toolbar.zoomLevel')}>
        {String(Math.round(zoom * 100))}%
      </span>
      <Tool
        icon="plus"
        label={t('workflow.toolbar.zoomIn')}
        onClick={() => {
          void flow.zoomIn();
        }}
      />
      <span className={styles.zoomRule} aria-hidden="true" />
      <Tool
        icon="fit"
        onClick={() => {
          void flow.fitView({ padding: 0.2, maxZoom: 1.1 });
        }}
      >
        {t('workflow.toolbar.fit')}
      </Tool>
    </div>
  );
}
