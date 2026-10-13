import { useEffect, useState } from 'react';
import { subscribe } from '../hooks/subscribe';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { useNow } from '@/features/agents/hooks/useNow';
import { clock } from '@/features/workflow/model/time';
import { subscribeToExecutionEvents } from '@/features/workspace/services/workspaceService';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import { failureText, stageName } from '../model/messages';
import type { SpecArtifact } from '../types';
import styles from './Specs.module.css';

/** What the runtime last reported, in words; kinds that say nothing to a person are left out. */
const EVENT_KEYS: Record<string, TranslationKey> = {
  starting_runtime: 'specs.run.event.starting_runtime',
  sending_prompt: 'specs.run.event.sending_prompt',
  waiting_for_model: 'specs.run.event.waiting_for_model',
  output_chunk: 'specs.run.event.output_chunk',
  tool_started: 'specs.run.event.tool_started',
  tool_completed: 'specs.run.event.tool_completed',
};

interface Props {
  artifact: SpecArtifact;
  agentName: string;
  onCancel: () => void;
  busy: boolean;
}

/** The agent's turn, as it really is: when it started, what it last reported, or why it failed. */
export function RunBanner({ artifact, agentName, onCancel, busy }: Props) {
  const { t } = useI18n();
  const generation = artifact.generation;
  const runId = generation.state === 'running' ? generation.runId : null;
  const [heard, setHeard] = useState<{ runId: string; kind: string } | null>(null);
  const last = heard?.runId === runId ? heard.kind : null;
  const now = useNow(1000);

  useEffect(() => {
    if (!runId) return;
    return subscribe(() =>
      subscribeToExecutionEvents((event) => {
        if (event.executionId === runId) setHeard({ runId, kind: event.kind });
      }),
    );
  }, [runId]);

  if (generation.state === 'running') {
    return (
      <div className={styles.banner} data-tone="running" role="status">
        <Icon name="spinner" size={16} />
        <div className={styles.bannerText}>
          <strong>
            {t('specs.run.writing', { agent: agentName, stage: stageName(t, artifact.id) })}
          </strong>
          <span>
            {t(`specs.run.mode.${generation.mode}`)} · {clock(now - generation.startedAt)}
            {last && EVENT_KEYS[last] ? ` · ${t(EVENT_KEYS[last])}` : ''}
          </span>
        </div>
        <Button variant="secondary" disabled={busy} onClick={onCancel}>
          {t('specs.run.cancel')}
        </Button>
      </div>
    );
  }
  if (generation.state === 'failed') {
    const failure = failureText(t, generation.message);
    return (
      <div className={styles.banner} data-tone="danger" role="alert">
        <Icon name="warning" size={16} />
        <div className={styles.bannerText}>
          <strong>
            {generation.interrupted ? t('specs.run.interrupted') : t('specs.run.failed')}
          </strong>
          <span>{failure.text}</span>
          {failure.detail && (
            <details>
              <summary>{t('specs.run.details')}</summary>
              <pre className={styles.source}>{failure.detail}</pre>
            </details>
          )}
          {artifact.revisions.length > 0 && <span>{t('specs.run.kept')}</span>}
        </div>
      </div>
    );
  }
  return null;
}
