import { useEffect, useRef, useState } from 'react';
import type { RuntimeCapabilitiesDto, TerminalSnapshotDto } from '@/lib/tauri/commands';
import { useT } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import { useProcessControls } from '../hooks/useProcessControls';
import type { AgentRun, ProcessState } from '../model/agentRuns';
import { formatSeconds } from '../model/activity';
import type { TerminalHub } from '../model/terminalHub';
import {
  getExecutionTerminal,
  resizeTerminal,
  sendTerminalInput,
} from '../services/workspaceService';
import styles from './Terminal.module.css';
import { TerminalView, type TerminalViewHandle } from './TerminalView';

/** How long an interrupt may take before the panel points out that the process can be ended. */
const STILL_RUNNING_AFTER_MS = 5000;

/**
 * Keystrokes and size changes are fire-and-forget: there is nothing useful to say to the user
 * about one that did not arrive (the process may have just ended).
 */
function attempt(call: () => Promise<void>): void {
  void (async () => {
    try {
      await call();
    } catch {
      // See above.
    }
  })();
}

interface Props {
  workspaceId: string;
  agentId: string;
  agentName: string;
  runtimeName: string;
  /** What the agent's runtime supports; the panel shows only what it really can. */
  capabilities: RuntimeCapabilitiesDto | undefined;
  run: AgentRun | undefined;
  process: ProcessState | undefined;
  hub: TerminalHub;
}

/**
 * The terminal tab of an agent: the real process of its latest execution. Chat is what the agent
 * says; this is what the process does. It controls an execution that already runs, under the
 * policy it was started with: there is no way to type a command of Atlas's here, and none to
 * start a process.
 */
export function TerminalPanel({
  workspaceId,
  agentId,
  agentName,
  runtimeName,
  capabilities,
  run,
  process,
  hub,
}: Props) {
  const t = useT();
  const view = useRef<TerminalViewHandle>(null);
  const executionId = run?.executionId ?? null;
  const ref = executionId ? { workspaceId, agentId, executionId } : null;
  const controls = useProcessControls(ref);
  // The core's snapshot of the process, tagged with the session it was read for.
  const [loaded, setLoaded] = useState<{
    key: string;
    value: TerminalSnapshotDto | null | 'failed';
  } | null>(null);
  const [autoScroll, setAutoScroll] = useState(true);
  const [searching, setSearching] = useState(false);
  const [query, setQuery] = useState('');
  const [overdueFor, setOverdueFor] = useState<string | null>(null);
  const sessionId = process?.processSessionId ?? null;
  const sessionKey = executionId && sessionId ? `${executionId}/${sessionId}` : null;

  // The core's snapshot (command, size, output so far) once there is a process to show.
  useEffect(() => {
    if (!executionId || !sessionKey) return;
    let cancelled = false;
    getExecutionTerminal({ workspaceId, agentId, executionId })
      .then((value) => {
        if (cancelled) return;
        if (value) hub.load(value);
        setLoaded({ key: sessionKey, value });
      })
      .catch(() => {
        if (!cancelled) setLoaded({ key: sessionKey, value: 'failed' });
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceId, agentId, executionId, sessionKey, hub]);
  // `undefined` while the snapshot for this session is being read.
  const snapshot = loaded?.key === sessionKey ? loaded.value : undefined;

  // An interrupt the process has not obeyed for a while: say that it can be ended.
  const stopping = process?.status === 'interrupting';
  useEffect(() => {
    if (!stopping || !sessionId) return;
    const timer = setTimeout(() => {
      setOverdueFor(sessionId);
    }, STILL_RUNNING_AFTER_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [stopping, sessionId]);
  const overdue = stopping && overdueFor === sessionId;

  if (capabilities && !capabilities.interactiveTerminal) {
    return <p className={styles.empty}>{t('terminal.unsupported', { runtime: runtimeName })}</p>;
  }
  if (!executionId || !process) {
    return <p className={styles.empty}>{t('terminal.empty')}</p>;
  }
  if (snapshot === 'failed') {
    return (
      <p role="alert" className={styles.empty}>
        {t('terminal.disconnected')}
      </p>
    );
  }

  const details = snapshot ?? null;
  const live = process.status !== 'exited';
  const canInput = (capabilities?.terminalInput ?? false) && (details?.inputEnabled ?? false);
  const canInterrupt = capabilities?.interrupt ?? true;
  const userControlled = process.userAction !== null;
  const duration =
    process.endedAt === null ? null : formatSeconds(String(process.endedAt - process.startedAt));
  function copy() {
    void view.current?.copy();
  }
  function clear() {
    view.current?.clear();
  }

  return (
    <div className={styles.panel}>
      <div className={styles.statusRow}>
        <span role="status" className={styles.connection} data-state={process.status}>
          <span aria-hidden="true">●</span>{' '}
          {live
            ? t('terminal.connected', { id: executionId })
            : `${t('terminal.exited')} · ${
                process.exitCode === null
                  ? t('terminal.exitSignal')
                  : t('terminal.exitCode', { code: process.exitCode })
              }${duration ? ` · ${t('terminal.duration', { duration })}` : ''}`}
        </span>
        {userControlled && <span className={styles.badge}>{t('terminal.userControlled')}</span>}
        {!canInput && <span className={styles.badge}>{t('terminal.readOnly')}</span>}
      </div>
      {process.status === 'interrupting' && (
        <p className={styles.hint}>
          {overdue ? t('terminal.stillRunning') : t('terminal.interrupting')}
        </p>
      )}
      {process.status === 'terminating' && (
        <p className={styles.hint}>{t('terminal.terminating')}</p>
      )}
      {!live && run?.status === 'cancelled' && (
        <p className={styles.hint}>{t('terminal.exitedCancelled')}</p>
      )}
      {details?.command && (
        <code className={styles.command}>$ {details.command.slice(0, 160)}</code>
      )}
      {snapshot !== undefined && (
        <TerminalView
          ref={view}
          executionId={executionId}
          hub={hub}
          label={t('terminal.view', { name: agentName })}
          readOnly={!canInput}
          autoScroll={autoScroll}
          onInterrupt={() => {
            if (live && canInterrupt) void controls.interrupt();
          }}
          onInput={
            canInput && live
              ? (data) => {
                  attempt(() => sendTerminalInput({ workspaceId, agentId, executionId }, data));
                }
              : undefined
          }
          onResize={
            capabilities?.terminalResize && live
              ? (cols, rows) => {
                  attempt(() => resizeTerminal({ workspaceId, agentId, executionId }, cols, rows));
                }
              : undefined
          }
        />
      )}
      {details?.truncated && <p className={styles.hint}>{t('terminal.truncated')}</p>}
      {controls.error !== undefined && (
        <p role="alert" className={styles.error}>
          {errorMessage(t, controls.error)}
        </p>
      )}
      <div
        className={styles.toolbar}
        role="toolbar"
        aria-label={t('terminal.controlsFor', { name: agentName })}
      >
        <button type="button" className={styles.tool} onClick={copy}>
          {t('terminal.copy')}
        </button>
        <button type="button" className={styles.tool} onClick={clear}>
          {t('terminal.clear')}
        </button>
        <button
          type="button"
          className={styles.tool}
          aria-expanded={searching}
          onClick={() => {
            setSearching((open) => !open);
          }}
        >
          {t('terminal.search')}
        </button>
        <label className={styles.check}>
          <input
            type="checkbox"
            checked={autoScroll}
            onChange={(event) => {
              setAutoScroll(event.target.checked);
            }}
          />{' '}
          {t('terminal.autoScroll')}
        </label>
        <span className={styles.spacer} />
        {live && canInterrupt && (
          <button
            type="button"
            className={styles.interrupt}
            title={t('terminal.interruptTitle')}
            disabled={process.status !== 'running'}
            onClick={() => {
              void controls.interrupt();
            }}
          >
            {t('terminal.interrupt')} ⌃C
          </button>
        )}
        {live && (
          <button
            type="button"
            className={styles.terminate}
            title={t('terminal.terminateTitle')}
            disabled={process.status === 'terminating'}
            onClick={() => {
              void controls.terminate();
            }}
          >
            {t('terminal.terminate')}
          </button>
        )}
      </div>
      {searching && (
        <form
          className={styles.searchRow}
          onSubmit={(event) => {
            event.preventDefault();
            view.current?.find(query, 'next');
          }}
        >
          <input
            type="search"
            className={styles.searchInput}
            aria-label={t('terminal.searchLabel')}
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
            }}
          />
          <button
            type="button"
            className={styles.tool}
            onClick={() => view.current?.find(query, 'previous')}
          >
            {t('terminal.searchPrevious')}
          </button>
          <button type="submit" className={styles.tool}>
            {t('terminal.searchNext')}
          </button>
        </form>
      )}
    </div>
  );
}
