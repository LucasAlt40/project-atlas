import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { useOptionalWorkspace } from '../hooks/WorkspaceProvider';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import type { TranslationKey } from '@/i18n';
import type { ExecutionWorktreeDto } from '@/lib/tauri/commands';
import styles from './Inspector.module.css';

/**
 * Why a merge that did not happen can be asked for again by the user: the situation is outside
 * the work itself and may have changed. A conflict, a policy that denies or a failed validation
 * cannot be waved through with a button.
 */
const RETRYABLE: readonly string[] = ['base_dirty', 'base_branch_changed', 'undetermined'];

export function canMerge(worktree: ExecutionWorktreeDto): boolean {
  return (
    worktree.status === 'completed' &&
    (worktree.mergeStatus === 'pending' ||
      (worktree.mergeStatus === 'blocked' && RETRYABLE.includes(worktree.blockReason ?? '')))
  );
}

function Fact({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className={styles.fact}>
      <dt>{label}</dt>
      <dd>{children}</dd>
    </div>
  );
}

interface Props {
  executionId: string;
  /** The agent's setting: whether this execution was meant to run isolated. */
  isolated: boolean;
  running: boolean;
}

/**
 * The Git side of an execution: where it started from, the branch and worktree it used, what
 * Git measured about the work, and what became of it. Everything shown is what the core
 * stored (or Git said); the only action is the user's explicit merge.
 */
export function GitDetails({ executionId, isolated, running }: Props) {
  const { t } = useI18n();
  const workspace = useOptionalWorkspace();
  const worktree = workspace?.worktrees[executionId];
  const [merging, setMerging] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!worktree && isolated && !running) return null;

  if (!worktree) {
    return (
      <section className={styles.gitSection} aria-label={t('git.title')}>
        <h3 className={styles.gitTitle}>{t('git.title')}</h3>
        <dl className={styles.facts}>
          <Fact label={t('git.worktree')}>
            {isolated ? t('git.worktree.creating') : t('git.worktree.notIsolated')}
          </Fact>
        </dl>
      </section>
    );
  }

  const { changes } = worktree;
  const kept = worktree.status !== 'cleaned' && worktree.mergeStatus !== 'merged';

  function merge(target: ExecutionWorktreeDto) {
    if (!workspace) return;
    setMerging(true);
    setError(null);
    workspace
      .mergeExecution({
        workspaceId: target.workspaceId,
        agentId: target.agentId,
        executionId: target.executionId,
      })
      .catch((e: unknown) => {
        setError(errorMessage(t, e));
      })
      .finally(() => {
        setMerging(false);
      });
  }

  return (
    <section className={styles.gitSection} aria-label={t('git.title')}>
      <h3 className={styles.gitTitle}>{t('git.title')}</h3>
      <dl className={styles.facts}>
        <Fact label={t('git.base')}>{worktree.baseBranch}</Fact>
        <Fact label={t('git.branch')}>{worktree.branchName}</Fact>
        <Fact label={t('git.worktree')}>
          {t('git.worktree.isolated')} · {t(`git.status.${worktree.status}` as TranslationKey)}
        </Fact>
        <Fact label={t('git.changes')}>
          {changes && (changes.filesChanged > 0 || changes.commitsAhead > 0)
            ? t('git.changes.summary', {
                files: changes.filesChanged,
                commits: changes.commitsAhead,
              })
            : t('git.changes.none')}
        </Fact>
        <Fact label={t('git.tests')}>
          {t(`git.tests.${worktree.validation}` as TranslationKey)}
        </Fact>
        <Fact label={t('git.merge')}>
          {t(`git.merge.${worktree.mergeStatus}` as TranslationKey)}
        </Fact>
      </dl>
      {worktree.blockReason && (
        <p className={styles.note}>{t(`git.reason.${worktree.blockReason}` as TranslationKey)}</p>
      )}
      {changes && changes.conflicts.length > 0 && (
        <>
          <p className={styles.note}>{t('git.conflicts')}:</p>
          <ul className={styles.gitFiles}>
            {changes.conflicts.map((file) => (
              <li key={file}>{file}</li>
            ))}
          </ul>
        </>
      )}
      {worktree.recommendation && (
        <p className={styles.note}>
          <strong>{t('git.recommended')}:</strong>{' '}
          {t(`git.recommendation.${worktree.recommendation}` as TranslationKey)}
        </p>
      )}
      {worktree.baseDirtyAtStart && <p className={styles.note}>{t('git.baseDirtyAtStart')}</p>}
      {kept && worktree.mergeStatus !== 'pending' && <p className={styles.note}>{t('git.kept')}</p>}
      {canMerge(worktree) && (
        <div className={styles.gitActions}>
          <Button
            disabled={merging}
            onClick={() => {
              merge(worktree);
            }}
          >
            {merging ? t('git.merging') : t('git.mergeButton', { base: worktree.baseBranch })}
          </Button>
        </div>
      )}
      {error && (
        <p role="alert" className={styles.gitError}>
          {error}
        </p>
      )}
    </section>
  );
}
