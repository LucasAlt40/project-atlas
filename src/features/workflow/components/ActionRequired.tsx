import { useEffect, useRef, useState } from 'react';
import { useNavigation } from '@/app/NavigationContext';
import { useWorkspace } from '@/features/workspace/hooks/WorkspaceProvider';
import { useI18n } from '@/i18n/I18nProvider';
import { usePendingInteractions } from '../hooks/usePendingInteractions';
import styles from './Interaction.module.css';

/**
 * In the header of every screen while an agent of the workspace is waiting for the person.
 * Choosing a question opens its run, where the question is shown in front of everything else.
 */
export function ActionRequired() {
  const { t } = useI18n();
  const workspace = useWorkspace();
  const { navigate } = useNavigation();
  const pending = usePendingInteractions(workspace.active?.id);
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return undefined;
    const onPointerDown = (event: MouseEvent) => {
      if (root.current && !root.current.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', onPointerDown);
    return () => {
      document.removeEventListener('mousedown', onPointerDown);
    };
  }, [open]);

  if (pending.length === 0) return null;
  const go = (index: number) => {
    const target = pending[index];
    if (!target) return;
    setOpen(false);
    navigate('workflow', {
      type: 'open-workflow',
      workflowId: target.workflowId,
      executionId: target.workflowExecutionId,
    });
  };

  return (
    <div className={styles.indicator} ref={root}>
      <button
        type="button"
        className={styles.indicatorButton}
        aria-haspopup={pending.length > 1 ? 'true' : undefined}
        aria-expanded={pending.length > 1 ? open : undefined}
        onClick={() => {
          if (pending.length === 1) go(0);
          else setOpen((value) => !value);
        }}
      >
        <span aria-hidden="true">⚠</span>
        {pending.length === 1
          ? t('interaction.indicatorOne')
          : t('interaction.indicatorMany', { count: pending.length })}
      </button>
      {open && pending.length > 1 && (
        <ul className={styles.indicatorList} aria-label={t('interaction.indicatorList')}>
          {pending.map((item, index) => (
            <li key={item.id}>
              <button
                type="button"
                className={styles.indicatorItem}
                onClick={() => {
                  go(index);
                }}
              >
                <span>{t('interaction.waiting', { step: item.stepLabel })}</span>
                <span className={styles.indicatorMeta}>{item.question}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
