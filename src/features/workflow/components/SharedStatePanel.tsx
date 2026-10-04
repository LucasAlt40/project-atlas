import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import { nodeLabel } from '../model/status';
import type { WorkflowRun } from '../types';
import styles from './Workflow.module.css';

/**
 * What the steps of a run share, compactly: artifacts, decisions, the places they changed and
 * the warnings. Never a transcript: steps do not talk to each other.
 */
export function SharedStatePanel({
  run,
  onOpenExecution,
}: {
  run: WorkflowRun;
  onOpenExecution: (executionId: string) => void;
}) {
  const { t } = useI18n();
  const { state } = run;
  const areas = [...new Set(Object.values(state.touchedAreas).flat())].sort();
  const files = [...new Set(Object.values(state.touchedFiles).flat())].sort();
  const empty =
    state.artifacts.length === 0 &&
    state.decisions.length === 0 &&
    areas.length === 0 &&
    files.length === 0 &&
    state.warnings.length === 0;

  return (
    <section className={styles.shared} aria-label={t('workflow.shared')}>
      <h4>{t('workflow.shared')}</h4>
      {empty && <p className={styles.muted}>{t('workflow.shared.empty')}</p>}
      {state.artifacts.length > 0 && (
        <>
          <h5>{t('workflow.shared.artifacts')}</h5>
          <ul>
            {state.artifacts.map((artifact) => (
              <li key={artifact.id}>
                <button
                  type="button"
                  className={styles.link}
                  title={artifact.summary}
                  onClick={() => {
                    onOpenExecution(artifact.executionId);
                  }}
                >
                  {artifact.name}
                </button>{' '}
                <span className={styles.muted}>
                  {t(`workflow.artifact.${artifact.type}` as TranslationKey)} ·{' '}
                  {nodeLabel(run, artifact.producerNodeId)}
                  {artifact.path ? ` · ${artifact.path}` : ''}
                </span>
              </li>
            ))}
          </ul>
        </>
      )}
      {state.decisions.length > 0 && (
        <>
          <h5>{t('workflow.shared.decisions')}</h5>
          <ul>
            {state.decisions.map((decision) => (
              <li key={decision.id} title={decision.rationale}>
                {decision.title || decision.decision}
              </li>
            ))}
          </ul>
        </>
      )}
      {(areas.length > 0 || files.length > 0) && (
        <>
          <h5>{t('workflow.shared.changed')}</h5>
          <p>{(areas.length > 0 ? areas : files).join(', ')}</p>
        </>
      )}
      {state.warnings.length > 0 && (
        <>
          <h5>{t('workflow.shared.warnings')}</h5>
          <ul>
            {state.warnings.map((warning) => (
              <li key={`${warning.nodeIds.join('+')}-${warning.paths.join(',')}`} role="status">
                {t('workflow.shared.overlap', {
                  a: nodeLabel(run, warning.nodeIds[0]),
                  b: nodeLabel(run, warning.nodeIds[1]),
                  paths: warning.paths.join(', '),
                })}
              </li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}
