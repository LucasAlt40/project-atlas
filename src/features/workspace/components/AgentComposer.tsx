import { useState, type KeyboardEvent, type SyntheticEvent } from 'react';
import { Button } from '@/components/ui/Button';
import { TaskContextPreview } from '@/features/harness/components/TaskContextPreview';
import { useT } from '@/i18n/I18nProvider';
import styles from './AgentCard.module.css';

interface Props {
  workspaceId: string;
  agentId: string;
  agentName: string;
  disabled: boolean;
  onSend: (content: string) => void;
}

export function AgentComposer({ workspaceId, agentId, agentName, disabled, onSend }: Props) {
  const t = useT();
  const [draft, setDraft] = useState('');

  function submit(event?: SyntheticEvent) {
    event?.preventDefault();
    const content = draft.trim();
    if (disabled || content === '') return;
    onSend(content);
    setDraft('');
  }

  function onKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === 'Enter' && !event.shiftKey) submit(event);
  }

  return (
    <>
      <TaskContextPreview workspaceId={workspaceId} agentId={agentId} task={draft} />
      <form className={styles.composer} onSubmit={submit}>
        <textarea
          aria-label={t('agent.composer.label', { name: agentName })}
          className={styles.input}
          rows={2}
          placeholder={t('agent.composer.placeholder')}
          value={draft}
          onChange={(e) => {
            setDraft(e.target.value);
          }}
          onKeyDown={onKeyDown}
        />
        <Button type="submit" disabled={disabled || draft.trim() === ''}>
          {t('agent.composer.send')}
        </Button>
      </form>
    </>
  );
}
