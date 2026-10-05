import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { useI18n } from '@/i18n/I18nProvider';
import type { RepairChoice, RepairProposal, Workflow } from '../types';
import styles from './Workflow.module.css';

interface Props {
  workflow: Workflow;
  proposals: RepairProposal[];
  busy: boolean;
  onCancel: () => void;
  onApply: (choices: RepairChoice[]) => void;
}

/**
 * The routes that cannot match their agent's contract, each with the outcome Atlas would pair
 * it with. Nothing changes until the person applies; the pairing is theirs to correct.
 */
export function RouteRepairDialog({ workflow, proposals, busy, onCancel, onApply }: Props) {
  const { t } = useI18n();
  const [chosen, setChosen] = useState<Record<string, string>>(() =>
    Object.fromEntries(
      proposals.flatMap((p) => p.edges.map((e) => [e.edgeId, e.suggested ?? ''] as const)),
    ),
  );
  const label = (nodeId: string) => workflow.nodes.find((n) => n.id === nodeId)?.label ?? nodeId;
  const ready = Object.values(chosen).every(Boolean);

  return (
    <Modal label={t('workflow.repair.title')} onClose={onCancel}>
      <h2>{t('workflow.repair.title')}</h2>
      <p className={styles.muted}>{t('workflow.repair.intro')}</p>
      {proposals.map((proposal) => (
        <section key={proposal.nodeId} className={styles.repairGroup}>
          <h3>{label(proposal.nodeId)}</h3>
          <p className={styles.muted}>
            {t('workflow.repair.agent', { agent: proposal.agent })} ·{' '}
            {t('workflow.repair.contract', { outcomes: proposal.declared.join(', ') })}
          </p>
          {proposal.edges.map((edge) => (
            <div key={edge.edgeId} className={styles.repairRow}>
              <div>
                <span className={styles.fieldLabel}>{t('workflow.repair.current')}</span>
                <code>
                  {t('workflow.repair.route', {
                    field: edge.current.field,
                    value: edge.current.value ?? '',
                    target: label(edge.targetNodeId),
                  })}
                </code>
              </div>
              <label>
                <span className={styles.fieldLabel}>{t('workflow.repair.suggested')}</span>
                <select
                  className={styles.control}
                  value={chosen[edge.edgeId] ?? ''}
                  onChange={(e) => {
                    setChosen((current) => ({ ...current, [edge.edgeId]: e.target.value }));
                  }}
                >
                  <option value="">{t('workflow.repair.choose')}</option>
                  {proposal.declared.map((outcome) => (
                    <option key={outcome} value={outcome}>
                      {`result.outcome = ${outcome}`}
                    </option>
                  ))}
                </select>
              </label>
            </div>
          ))}
        </section>
      ))}
      <p role="note" className={styles.warning}>
        {t('workflow.repair.note')}
      </p>
      <div className={styles.actions}>
        <Button variant="secondary" onClick={onCancel}>
          {t('workflow.repair.cancel')}
        </Button>
        <Button
          disabled={!ready || busy}
          onClick={() => {
            onApply(Object.entries(chosen).map(([edgeId, outcome]) => ({ edgeId, outcome })));
          }}
        >
          {t('workflow.repair.apply')}
        </Button>
      </div>
    </Modal>
  );
}
