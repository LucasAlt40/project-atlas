import type { RuntimeCapabilitiesDto } from '@/lib/tauri/commands';
import { useT } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import { useProcessControls } from '../hooks/useProcessControls';
import { activityLabel } from '../model/activity';
import type { AgentRun, ProcessState } from '../model/agentRuns';
import styles from './Terminal.module.css';

interface Props {
  workspaceId: string;
  agentId: string;
  run: AgentRun;
  process: ProcessState;
  capabilities: RuntimeCapabilitiesDto | undefined;
  onOpenTerminal: () => void;
}

/**
 * While an execution's process runs, wherever the card is looking: what it is doing now, a way
 * to its terminal and the interrupt. Terminate is not here: it lives in the terminal, where the
 * output that makes it necessary is.
 */
export function RunControls({
  workspaceId,
  agentId,
  run,
  process,
  capabilities,
  onOpenTerminal,
}: Props) {
  const t = useT();
  const controls = useProcessControls({ workspaceId, agentId, executionId: run.executionId });
  const last = run.activity.at(-1);
  return (
    <div className={styles.runBar}>
      <span className={styles.runStep}>
        {process.status === 'running' && last ? activityLabel(t, last) : t('agent.status.stopping')}
      </span>
      {(capabilities?.interactiveTerminal ?? true) && (
        <button type="button" className={styles.tool} onClick={onOpenTerminal}>
          {t('terminal.open')}
        </button>
      )}
      {(capabilities?.interrupt ?? true) && (
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
      {controls.error !== undefined && (
        <span role="alert" className={styles.error}>
          {errorMessage(t, controls.error)}
        </span>
      )}
    </div>
  );
}
