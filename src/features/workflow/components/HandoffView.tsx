import { Icon } from '@/components/ui/Icon';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import { shortId } from '@/features/workspace/model/inspection';
import { divergence } from '../model/review';
import { nodeLabel } from '../model/status';
import { agoText, clock } from '../model/time';
import type { FileChange, Handoff, WorkflowRun } from '../types';
import type { ResultFindingDto } from '@/lib/tauri/commands';
import styles from './Workflow.module.css';

const MARK: Record<FileChange['status'], string> = {
  added: 'A',
  modified: 'M',
  deleted: 'D',
  renamed: 'R',
};

export function FileLine({ file }: { file: FileChange }) {
  const { t } = useI18n();
  return (
    <span className={styles.fileLine}>
      <span
        className={styles.fileMark}
        data-status={file.status}
        title={t(`integration.file.${file.status}` as TranslationKey)}
      >
        {MARK[file.status]}
      </span>{' '}
      <span className={styles.filePath}>
        {file.oldPath ? `${file.oldPath} → ` : ''}
        {file.path}
      </span>{' '}
      <span className={styles.muted}>
        {file.binary
          ? t('integration.binary')
          : `+${String(file.additions ?? 0)} −${String(file.deletions ?? 0)}`}
      </span>
    </span>
  );
}

/** One finding as one line: its severity and category, its title, where it is and what it says. */
export function FindingText({ finding: f }: { finding: ResultFindingDto }) {
  return (
    <>
      [{f.severity}
      {f.category ? ` / ${f.category}` : ''}] {f.title ? <strong>{f.title}: </strong> : null}
      {f.description}
      {f.file && (
        <span className={styles.filePath}>
          {' '}
          ({f.file}
          {f.line ? `:${String(f.line)}` : ''})
        </span>
      )}
      {f.evidence && <span className={styles.muted}> — {f.evidence}</span>}
    </>
  );
}

/** A changed file as a row: its mark, name and folder, and what changed in it. */
export function FileRow({ file }: { file: FileChange }) {
  const { t } = useI18n();
  const slash = file.path.lastIndexOf('/');
  const name = slash >= 0 ? file.path.slice(slash + 1) : file.path;
  const folder = slash >= 0 ? file.path.slice(0, slash) : '';
  return (
    <li className={styles.fileRow}>
      <span
        className={styles.fileMark}
        data-status={file.status}
        title={t(`integration.file.${file.status}` as TranslationKey)}
      >
        {MARK[file.status]}
      </span>
      <span className={styles.fileRowText}>
        <span className={styles.fileRowName}>{name}</span>
        {folder && <span className={styles.fileRowFolder}>{folder}</span>}
      </span>
      <span className={styles.fileRowCount}>
        {file.binary ? (
          t('integration.binary')
        ) : (
          <>
            <span data-sign="add">+{String(file.additions ?? 0)}</span>{' '}
            <span data-sign="del">−{String(file.deletions ?? 0)}</span>
          </>
        )}
      </span>
    </li>
  );
}

const SEVERITY_TONE: Record<string, string> = {
  critical: 'high',
  high: 'high',
  medium: 'medium',
  low: 'low',
  info: 'low',
};

/** One finding as a card: how serious and where, what it is, what it says and the evidence. */
export function FindingCard({ finding: f }: { finding: ResultFindingDto }) {
  return (
    <li className={styles.findingCard} data-tone={SEVERITY_TONE[f.severity.toLowerCase()] ?? 'low'}>
      <span className={styles.findingHead}>
        <span className={styles.findingTag}>
          {f.severity}
          {f.category ? ` · ${f.category}` : ''}
        </span>
        {f.title && <strong>{f.title}</strong>}
        {f.file && (
          <span className={styles.findingWhere}>
            {f.file}
            {f.line ? `:${String(f.line)}` : ''}
          </span>
        )}
      </span>
      <span className={styles.findingText}>{f.description}</span>
      {f.evidence && <span className={styles.findingEvidence}>{f.evidence}</span>}
    </li>
  );
}

/**
 * What one step handed to the next, laid out as the design draws it: who gave it to whom, the
 * result the step declared, what is now shared (decisions, artifacts, the files Git saw change),
 * what it validated, and the next action it suggested. Everything here was put together by the
 * orchestrator; what the agent merely claimed is shown apart, as a claim.
 */
export function HandoffView({
  run,
  handoff,
  onOpenExecution,
  index,
  agentName,
}: {
  run: WorkflowRun;
  handoff: Handoff;
  onOpenExecution: (executionId: string) => void;
  /** Its number in the run, from 1. */
  index?: number | undefined;
  /** The name of an agent, for the two ends of the handoff. */
  agentName?: (agentId: string) => string;
}) {
  const { t, language } = useI18n();
  const { claimedOnly: claimed, detectedOnly } = divergence(handoff);
  const agentOf = (nodeId: string) => {
    const node = run.workflow.nodes.find((n) => n.id === nodeId);
    return node?.type === 'agent' && agentName ? agentName(node.agentId) : null;
  };
  const attempt = run.nodes[handoff.fromNodeId]?.attempts.find(
    (a) => a.executionId === handoff.fromExecutionId,
  );
  const took = attempt?.completedAt ? clock(attempt.completedAt - attempt.startedAt) : null;
  const unit = (nodeId: string) => nodeLabel(run, nodeId);
  const shared =
    handoff.decisions.length +
    handoff.artifacts.length +
    handoff.changedFiles.length +
    handoff.uncommittedFiles.length;
  const heading = (number: number, title: string, meta?: string) => {
    return (
      <h5 className={styles.cardLabel}>
        <span>
          {String(number).padStart(2, '0')} · {title}
        </span>
        {meta && <span className={styles.cardLabelMeta}>{meta}</span>}
      </h5>
    );
  };
  return (
    <article
      className={styles.handoff}
      aria-label={t('handoff.title', {
        from: unit(handoff.fromNodeId),
        to: unit(handoff.toNodeId),
      })}
    >
      <header className={styles.handoffHead}>
        <span className={styles.handoffMeta}>
          <strong>HANDOFF{index === undefined ? '' : ` #${String(index)}`}</strong>
          <span>· {agoText(handoff.createdAt, language)}</span>
          {took && (
            <span>
              · <Icon name="pulse" size={11} /> {took} {t('handoff.step')}
            </span>
          )}
        </span>
        <span className={styles.chip} data-tone={handoff.status}>
          {t(`handoff.status.${handoff.status}` as TranslationKey)}
        </span>
      </header>
      <h4 className={styles.handoffTitle}>
        {unit(handoff.fromNodeId)} → {unit(handoff.toNodeId)}{' '}
        <span className={styles.muted}>{t('handoff.pass', { n: handoff.iteration })}</span>
      </h4>
      <div className={styles.agentPair}>
        {(['from', 'to'] as const).map((end) => {
          const nodeId = end === 'from' ? handoff.fromNodeId : handoff.toNodeId;
          return (
            <div key={end} className={styles.agentBox}>
              <span className={styles.cardLabel}>
                {t(end === 'from' ? 'handoff.sender' : 'handoff.receiver')}
              </span>
              <strong>{agentOf(nodeId) ?? unit(nodeId)}</strong>
              {agentOf(nodeId) && <span className={styles.muted}>{unit(nodeId)}</span>}
            </div>
          );
        })}
      </div>

      {heading(
        1,
        t('handoff.section.contract'),
        `status: ${handoff.status}${handoff.outcome ? ` · outcome: ${handoff.outcome.toUpperCase()}` : ''}`,
      )}
      <p className={styles.handoffStatus} data-status={handoff.status}>
        {handoff.kind === 'failure' ? t('handoff.failedStep') : t('handoff.result')}:{' '}
        <strong>{t(`handoff.status.${handoff.status}` as TranslationKey)}</strong>
        {' · '}
        {t('handoff.executionStatus')}:{' '}
        <strong>
          {handoff.kind === 'failure'
            ? t('handoff.executionStatus.failed')
            : t('handoff.executionStatus.completed')}
        </strong>
        {handoff.outcome && (
          <>
            {' · '}
            {t('handoff.outcome')}:{' '}
            <strong data-outcome={handoff.outcome}>{handoff.outcome.toUpperCase()}</strong>
          </>
        )}
      </p>
      <p className={styles.cardBlock}>
        {handoff.summary === '' ? (handoff.failure ?? t('handoff.nothing')) : handoff.summary}
      </p>
      {handoff.failure && handoff.summary && <p className={styles.cardBlock}>{handoff.failure}</p>}

      {heading(
        2,
        t('handoff.section.shared'),
        shared > 0 ? t('handoff.sharedCount', { n: shared }) : undefined,
      )}
      {handoff.decisions.map((d) => (
        <div key={`${d.title}-${d.decision}`} className={styles.decisionRow} title={d.rationale}>
          <Icon name="branch" size={14} />
          <span>
            <strong>{d.title ? `${d.title}: ` : ''}</strong>
            {d.decision}
          </span>
        </div>
      ))}
      {handoff.artifacts.length > 0 && (
        <>
          <h5 className={styles.cardLabel}>{t('handoff.artifacts')}</h5>
          <ul className={styles.rowList}>
            {handoff.artifacts.map((a) => (
              <li key={a.id} title={a.summary} className={styles.fileRow}>
                <Icon name="stack" size={14} />
                <span className={styles.fileRowText}>
                  <span className={styles.fileRowName}>{a.name}</span>
                  {a.path && <span className={styles.fileRowFolder}>{a.path}</span>}
                </span>
              </li>
            ))}
          </ul>
        </>
      )}
      <h5 className={styles.cardLabel}>{t('handoff.changedFiles')}</h5>
      {handoff.changedFiles.length === 0 && handoff.uncommittedFiles.length === 0 ? (
        <p className={styles.muted}>{t('handoff.noFiles')}</p>
      ) : (
        <ul className={styles.rowList}>
          {handoff.changedFiles.map((file) => (
            <FileRow key={file.path} file={file} />
          ))}
          {handoff.uncommittedFiles.map((path) => (
            <li key={`u-${path}`} className={styles.fileRow}>
              <span className={styles.fileRowText}>
                <span className={styles.fileRowName}>{path}</span>
              </span>
              <span className={styles.muted}>{t('handoff.uncommitted')}</span>
            </li>
          ))}
        </ul>
      )}
      {claimed.length > 0 && (
        <p className={styles.muted}>{t('handoff.claimed', { files: claimed.join(', ') })}</p>
      )}
      {detectedOnly.length > 0 && handoff.reportedFiles.length > 0 && (
        <p className={styles.muted}>
          {t('handoff.detectedOnly', { files: detectedOnly.join(', ') })}
        </p>
      )}
      {run.integration.branch && (
        <div className={styles.branchRow}>
          <span className={styles.cardLabel}>{t('handoff.branch')}</span>
          <code>{run.integration.branch}</code>
        </div>
      )}

      {handoff.validation &&
        heading(
          3,
          t('handoff.section.evidence'),
          handoff.validation.findings.length > 0
            ? t('handoff.findingsCount', { n: handoff.validation.findings.length })
            : undefined,
        )}
      {handoff.validation && (
        <>
          <p className={styles.cardBlock}>
            <strong>{t(`handoff.status.${handoff.validation.status}` as TranslationKey)}</strong>
            {handoff.validation.summary ? ` — ${handoff.validation.summary}` : ''}
          </p>
          <ul className={styles.rowList}>
            {handoff.validation.findings.map((f) => (
              <FindingCard key={`${f.category}-${f.title}-${f.description}`} finding={f} />
            ))}
          </ul>
        </>
      )}

      {handoff.instructions && (
        <>
          {heading(4, t('handoff.section.next'), t('handoff.suggestionTag'))}
          <p className={styles.cardBlock}>{handoff.instructions}</p>
          <p className={styles.muted}>{t('handoff.suggestedNote')}</p>
        </>
      )}

      <p className={styles.handoffFoot}>
        <span className={styles.cardLabel}>{t('handoff.execution')}</span>
        <button
          type="button"
          className={styles.link}
          onClick={() => {
            onOpenExecution(handoff.fromExecutionId);
          }}
        >
          {shortId(handoff.fromExecutionId)}
        </button>
      </p>
    </article>
  );
}

export function HandoffList({
  run,
  handoffs,
  onOpenExecution,
  agentName,
}: {
  run: WorkflowRun;
  handoffs: Handoff[];
  onOpenExecution: (executionId: string) => void;
  agentName?: (agentId: string) => string;
}) {
  const { t } = useI18n();
  if (handoffs.length === 0) return <p className={styles.muted}>{t('handoff.none')}</p>;
  return (
    <div className={styles.handoffList}>
      {handoffs.map((handoff) => (
        <HandoffView
          key={handoff.id}
          run={run}
          handoff={handoff}
          index={run.handoffs.findIndex((h) => h.id === handoff.id) + 1 || undefined}
          {...(agentName ? { agentName } : {})}
          onOpenExecution={onOpenExecution}
        />
      ))}
    </div>
  );
}
