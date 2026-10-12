import { useT } from '@/i18n/I18nProvider';
import { RUN_LOOK, STEP_LOOK } from './labels';
import type { NodeStatus, RunStatus } from './types';
import styles from './Mobile.module.css';

/** A state as a glyph, words and a tone: it reads the same without colour. */
export function RunChip({ status }: { status: RunStatus }) {
  const t = useT();
  const look = RUN_LOOK[status];
  return (
    <span className={styles.chip} data-tone={look.tone}>
      <span aria-hidden="true">{look.glyph}</span>
      {t(look.key)}
    </span>
  );
}

export function StepChip({ status }: { status: NodeStatus }) {
  const t = useT();
  const look = STEP_LOOK[status];
  return (
    <span className={styles.chip} data-tone={look.tone}>
      <span aria-hidden="true">{look.glyph}</span>
      {t(look.key)}
    </span>
  );
}
