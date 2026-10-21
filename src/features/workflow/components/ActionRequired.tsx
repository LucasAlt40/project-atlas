import { useEffect, useRef, useState } from 'react';
import { useNavigation } from '@/app/NavigationContext';
import { useWorkspace } from '@/features/workspace/hooks/WorkspaceProvider';
import { useI18n } from '@/i18n/I18nProvider';
import { usePendingInteractions } from '../hooks/usePendingInteractions';
import { HeaderChip, headerPopover } from '@/components/layout/HeaderChip';

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
    <div className={headerPopover.root} ref={root}>
      <HeaderChip
        tone="warning"
        count={pending.length}
        aria-haspopup={pending.length > 1 ? 'true' : undefined}
        aria-expanded={pending.length > 1 ? open : undefined}
        onClick={() => {
          if (pending.length === 1) go(0);
          else setOpen((value) => !value);
        }}
      >
        {pending.length === 1
          ? t('interaction.indicatorOne')
          : t('interaction.indicatorMany', { count: pending.length })}
      </HeaderChip>
      {open && pending.length > 1 && (
        <ul className={headerPopover.list} aria-label={t('interaction.indicatorList')}>
          {pending.map((item, index) => (
            <li key={item.id}>
              <button
                type="button"
                className={headerPopover.item}
                onClick={() => {
                  go(index);
                }}
              >
                <span>{t('interaction.waiting', { step: item.stepLabel })}</span>
                <span className={headerPopover.meta}>{item.question}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
