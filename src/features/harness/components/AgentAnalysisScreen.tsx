import { useEffect, useState } from 'react';
import { Icon } from '@/components/ui/Icon';
import { useNow } from '@/features/agents/hooks/useNow';
import { formatDuration } from '@/features/usage/model/format';
import {
  interruptExecution,
  subscribeToHarnessProgress,
} from '@/features/workspace/services/workspaceService';
import { useT } from '@/i18n/I18nProvider';
import type { HarnessProgressDto } from '@/lib/tauri/commands';
import type { TranslationKey } from '@/i18n';
import styles from './Review.module.css';

interface Props {
  workspaceId: string;
  agentId: string;
  agentName: string;
  /** Project folder -> name, when several projects are analysed at once: one tab for each. */
  projects?: Record<string, string>;
}

interface Tool {
  id: number;
  name: string;
  startedAt: number;
  /** How long the call took, once it finished. */
  ms?: number;
}

/** One agent process reading one project. */
interface Run {
  executionId: string;
  path: string;
  startedAt: number;
  phase: string;
  tools: Tool[];
  narration: string;
  /** Counts the tools of this run, for their keys. */
  counter: number;
}

const PHASES = new Set(['starting', 'sending', 'waiting']);
/** What of the agent's narration is kept on screen. */
const MAX_NARRATION = 900;

/**
 * Shown over everything while an agent analyses the project (or, with `projects`, several of
 * them at once, one tab each): who is working, for how long, what it is doing right now and what
 * it says. It cannot be dismissed: the only way out is to wait or to stop the agent, so nobody
 * edits a half-analysed project's Harness by accident.
 */
export function AgentAnalysisScreen({ workspaceId, agentId, agentName, projects }: Props) {
  const t = useT();
  const [startedAt] = useState(() => Date.now());
  const now = useNow(1000);
  const [runs, setRuns] = useState<Run[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [stopping, setStopping] = useState(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void subscribeToHarnessProgress((update) => {
      if (update.workspaceId !== workspaceId || update.agentId !== agentId) return;
      setRuns((list) => {
        const known = list.some((run) => run.executionId === update.executionId);
        const base: Run[] = known
          ? list
          : [
              ...list,
              {
                executionId: update.executionId,
                path: update.projectPath,
                startedAt: Date.now(),
                phase: 'starting',
                tools: [],
                narration: '',
                counter: 0,
              },
            ];
        return base.map((run) =>
          run.executionId === update.executionId ? advance(run, update) : run,
        );
      });
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
    if (runs.length === 0) return;
    setStopping(true);
    // The analysis returns on its own once the process ends, with the reason.
    void Promise.all(
      runs.map((run) => interruptExecution({ workspaceId, agentId, executionId: run.executionId })),
    ).catch(() => {
      setStopping(false);
    });
  }

  const many = projects !== undefined && Object.keys(projects).length > 1;
  const run = runs.find((r) => r.executionId === selectedId) ?? runs[0];
  const phase = run?.phase ?? 'starting';
  const tools = run?.tools ?? [];
  const narration = run?.narration ?? '';

  const lines = narration === '' ? [] : narration.split('\n');
  const reads = tools.filter((tool) => tool.name.startsWith('Read')).length;

  return (
    <div className={styles.screen}>
      <div
        className={styles.screenCard}
        role="dialog"
        aria-modal="true"
        aria-label={t('harness.agentScreen.title', { agent: agentName })}
      >
        <div className={styles.screenHead}>
          <div className={styles.screenTop}>
            <p role="status" className={styles.live}>
              <span className={styles.ping} aria-hidden="true" />
              {t(`harness.agentScreen.phase.${phase}` as TranslationKey)} ·{' '}
              {formatDuration(now - (run?.startedAt ?? startedAt))}
            </p>
            <span className={styles.telemetry}>
              <Icon name="stack" size={13} />
              {t('harness.agentScreen.files', { count: reads })}
              <span aria-hidden="true">·</span>
              {t('harness.agentScreen.calls', { count: tools.length })}
            </span>
          </div>
          <h2 className={styles.screenTitle}>
            {t('harness.agentScreen.title', { agent: agentName })}
          </h2>
          <p className={styles.screenExplain}>{t('harness.agentScreen.explain')}</p>
          <div className={styles.bar} aria-hidden="true" />
        </div>

        {many && (
          <div role="tablist" aria-label={t('harness.multi.tabs')} className={styles.tabs}>
            {runs.map((r) => (
              <button
                key={r.executionId}
                type="button"
                role="tab"
                aria-selected={r.executionId === run?.executionId}
                className={styles.tab}
                onClick={() => {
                  setSelectedId(r.executionId);
                }}
              >
                {projects[r.path] ?? r.path}
              </button>
            ))}
          </div>
        )}

        <div className={styles.screenBody}>
          <section className={styles.section} aria-label={t('harness.agentScreen.doing')}>
            <h3 className={styles.sectionHead}>
              {t('harness.agentScreen.doing')}
              <span className={styles.sectionNote}>
                <span className={styles.dot} aria-hidden="true" />
                {t('harness.agentScreen.trace')}
              </span>
            </h3>
            {tools.length === 0 ? (
              <p className={styles.empty}>{t('harness.agentScreen.noTools')}</p>
            ) : (
              <ul className={styles.trace}>
                {tools.map((tool) => {
                  const [verb = '', ...rest] = tool.name.split(' ');
                  const args = rest.join(' ');
                  const running = tool.ms === undefined;
                  return (
                    <li key={tool.id} className={styles.traceRow} data-running={running}>
                      <span className={styles.traceMain}>
                        <span className={running ? styles.spin : styles.toneInfo}>
                          <Icon name={running ? 'refresh' : 'endNode'} size={15} />
                        </span>
                        <span className={styles.traceVerb}>{verb}</span>
                        {args !== '' && <span className={styles.traceArgs}>{args}</span>}
                      </span>
                      <span className={styles.traceMeta}>
                        {running ? t('harness.agentScreen.running') : `${String(tool.ms)}ms`}
                      </span>
                    </li>
                  );
                })}
              </ul>
            )}
          </section>

          {narration !== '' && (
            <section className={styles.section} aria-label={t('harness.agentScreen.says')}>
              <h3 className={styles.sectionHead}>
                {t('harness.agentScreen.stream')}
                <span className={styles.sectionNote}>
                  <span className={[styles.chip, styles.chipInfo].join(' ')}>
                    <span className={styles.dot} aria-hidden="true" />
                    {t('harness.agentScreen.live')}
                  </span>
                </span>
              </h3>
              <div className={styles.stream}>
                <div className={styles.streamTabs}>
                  <span className={styles.streamTitle}>
                    <Icon name="code" size={14} />
                    {t('harness.agentScreen.says')}
                  </span>
                  <span>UTF-8</span>
                </div>
                <div className={styles.streamBody}>
                  {lines.map((line, index) => (
                    <div key={index} className={styles.streamLine}>
                      <span className={styles.streamNo} aria-hidden="true">
                        {String(index + 1).padStart(2, '0')}
                      </span>
                      <span className={styles.streamText}>{line}</span>
                    </div>
                  ))}
                </div>
              </div>
            </section>
          )}
        </div>

        <div className={styles.screenFooter}>
          <button
            type="button"
            className={styles.stopButton}
            disabled={runs.length === 0 || stopping}
            onClick={stop}
          >
            <Icon name="stop" size={15} />
            {stopping ? t('harness.agentScreen.stopping') : t('harness.agentScreen.stop')}
          </button>
          <button type="button" className={styles.waitButton} disabled>
            <span className={styles.spin}>
              <Icon name="spinner" size={14} />
            </span>
            {t('harness.agentScreen.waitReview')}
          </button>
        </div>
      </div>
    </div>
  );
}

/** What one progress event changes in a run. */
function advance(run: Run, update: HarnessProgressDto): Run {
  switch (update.kind) {
    case 'step':
      return PHASES.has(update.text) ? { ...run, phase: update.text } : run;
    case 'tool_started': {
      const counter = run.counter + 1;
      return {
        ...run,
        counter,
        phase: 'working',
        tools: [...run.tools, { id: counter, name: update.text, startedAt: Date.now() }].slice(-12),
      };
    }
    case 'tool_completed': {
      const index = run.tools.findLastIndex(
        (tool) => tool.ms === undefined && tool.name === update.text,
      );
      return index < 0
        ? run
        : {
            ...run,
            tools: run.tools.map((tool, i) =>
              i === index ? { ...tool, ms: Date.now() - tool.startedAt } : tool,
            ),
          };
    }
    case 'output':
      return {
        ...run,
        phase: 'working',
        narration: (run.narration + update.text).slice(-MAX_NARRATION),
      };
    case 'started':
      return run;
  }
}
