import { useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { useNow } from '@/features/agents/hooks/useNow';
import { formatDuration } from '@/features/usage/model/format';
import {
  interruptExecution,
  subscribeToHarnessProgress,
} from '@/features/workspace/services/workspaceService';
import { useT } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import styles from './Harness.module.css';

interface Props {
  workspaceId: string;
  agentId: string;
  agentName: string;
}

interface Tool {
  id: number;
  name: string;
  done: boolean;
}

const PHASES = new Set(['starting', 'sending', 'waiting']);
/** What of the agent's narration is kept on screen. */
const MAX_NARRATION = 900;

/**
 * Shown over everything while an agent analyses the project: who is working, for how long, what
 * it is doing right now and what it says. It cannot be dismissed: the only way out is to wait or
 * to stop the agent, so nobody edits a half-analysed project's Harness by accident.
 */
export function AgentAnalysisScreen({ workspaceId, agentId, agentName }: Props) {
  const t = useT();
  const [startedAt] = useState(() => Date.now());
  const now = useNow(1000);
  const [executionId, setExecutionId] = useState<string | null>(null);
  const [phase, setPhase] = useState('starting');
  const [tools, setTools] = useState<Tool[]>([]);
  const [narration, setNarration] = useState('');
  const [stopping, setStopping] = useState(false);
  const counter = useRef(0);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void subscribeToHarnessProgress((update) => {
      if (update.workspaceId !== workspaceId || update.agentId !== agentId) return;
      setExecutionId(update.executionId);
      switch (update.kind) {
        case 'step':
          if (PHASES.has(update.text)) setPhase(update.text);
          break;
        case 'tool_started':
          counter.current += 1;
          setTools((list) =>
            [...list, { id: counter.current, name: update.text, done: false }].slice(-12),
          );
          setPhase('working');
          break;
        case 'tool_completed':
          setTools((list) => {
            const index = list.findLastIndex((tool) => !tool.done && tool.name === update.text);
            return index < 0
              ? list
              : list.map((tool, i) => (i === index ? { ...tool, done: true } : tool));
          });
          break;
        case 'output':
          setPhase('working');
          setNarration((text) => (text + update.text).slice(-MAX_NARRATION));
          break;
        case 'started':
          break;
      }
    }).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [workspaceId, agentId]);

  function stop() {
    if (!executionId) return;
    setStopping(true);
    // The analysis returns on its own once the process ends, with the reason.
    void interruptExecution({ workspaceId, agentId, executionId }).catch(() => {
      setStopping(false);
    });
  }

  return (
    <div className={styles.blocker}>
      <div
        className={styles.blockerCard}
        role="dialog"
        aria-modal="true"
        aria-label={t('harness.agentScreen.title', { agent: agentName })}
      >
        <h2 className={styles.title}>{t('harness.agentScreen.title', { agent: agentName })}</h2>
        <p className={styles.muted}>{t('harness.agentScreen.explain')}</p>
        <p role="status" className={styles.phase}>
          <span className={styles.pulse} aria-hidden="true" />
          {t(`harness.agentScreen.phase.${phase}` as TranslationKey)} ·{' '}
          {formatDuration(now - startedAt)}
        </p>

        <section aria-label={t('harness.agentScreen.doing')}>
          <h3 className={styles.heading}>{t('harness.agentScreen.doing')}</h3>
          {tools.length === 0 ? (
            <p className={styles.muted}>{t('harness.agentScreen.noTools')}</p>
          ) : (
            <ul className={styles.list}>
              {tools.map((tool) => (
                <li key={tool.id}>
                  <span className={tool.done ? styles.ok : styles.maybe}>
                    {tool.done ? '✓' : '…'}
                  </span>{' '}
                  <code>{tool.name}</code>
                </li>
              ))}
            </ul>
          )}
        </section>

        {narration !== '' && (
          <section aria-label={t('harness.agentScreen.says')}>
            <h3 className={styles.heading}>{t('harness.agentScreen.says')}</h3>
            <p className={styles.narration}>{narration}</p>
          </section>
        )}

        <div className={styles.actions}>
          <Button disabled={!executionId || stopping} onClick={stop}>
            {stopping ? t('harness.agentScreen.stopping') : t('harness.agentScreen.stop')}
          </Button>
        </div>
      </div>
    </div>
  );
}
