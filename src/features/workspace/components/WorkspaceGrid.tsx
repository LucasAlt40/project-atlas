import type { CSSProperties, ReactNode } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { useT } from '@/i18n/I18nProvider';
import { gridCells } from '../model/grid';
import type { Workspace } from '../types';
import styles from './Workspace.module.css';

interface Props {
  workspace: Workspace;
  renderAgent: (agentId: string) => ReactNode;
  /** Offered in an empty cell: the same action as the header's "Add agent". */
  onAdd?: () => void;
}

/**
 * Lays workspace items out on the grid by position. Each placed agent is rendered
 * independently by `renderAgent`; empty cells stay available.
 */
export function WorkspaceGrid({ workspace, renderAgent, onAdd }: Props) {
  const t = useT();
  return (
    <div
      className={styles.grid}
      style={
        {
          '--columns': workspace.layout.columns,
          '--rows': workspace.layout.rows,
        } as CSSProperties
      }
    >
      {gridCells(workspace).map((cell) => (
        <div
          key={`${String(cell.row)}-${String(cell.column)}`}
          className={cell.agentId ? styles.cell : styles.slot}
          data-row={cell.row}
          data-column={cell.column}
        >
          {cell.agentId ? (
            renderAgent(cell.agentId)
          ) : (
            <>
              <span className={styles.slotTile} aria-hidden="true">
                <Icon name="plus" size={20} />
              </span>
              <span className={styles.slotText}>{t('workspace.available')}</span>
              {onAdd && (
                <Button variant="secondary" onClick={onAdd}>
                  <Icon name="plus" size={16} />
                  {t('workspace.slot.cta')}
                </Button>
              )}
            </>
          )}
        </div>
      ))}
    </div>
  );
}
