import { useState, type KeyboardEvent, type SyntheticEvent } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { TaskContextPreview } from '@/features/harness/components/TaskContextPreview';
import { useT } from '@/i18n/I18nProvider';
import type { WorkspaceProjectDto } from '@/lib/tauri/commands';
import type { ScopeChoice } from '../model/scope';
import { ScopePicker } from './ScopePicker';
import styles from './AgentCard.module.css';

/** The projects of the workspace and what this agent was given: its scope picker. */
export interface AgentScope {
  projects: WorkspaceProjectDto[];
  choice: ScopeChoice;
  onChange: (choice: ScopeChoice) => void;
}

interface Props {
  workspaceId: string;
  agentId: string;
  agentName: string;
  disabled: boolean;
  onSend: (content: string) => void;
  scope?: AgentScope;
}

export function AgentComposer({ workspaceId, agentId, agentName, disabled, onSend, scope }: Props) {
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
      <form className={styles.composerPanel} onSubmit={submit}>
        {scope && scope.projects.length > 1 && (
          <ScopePicker
            projects={scope.projects}
            choice={scope.choice}
            onChange={scope.onChange}
            disabled={disabled}
          />
        )}
        <span className={styles.composerHint}>
          <Icon name="send" size={14} />
          {t('agent.composer.hint', { name: agentName })}
        </span>
        <div className={styles.composer}>
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
            <Icon name="send" size={14} />
          </Button>
        </div>
      </form>
    </>
  );
}
