import { Icon } from './Icon';
import styles from './Steps.module.css';

export interface Step {
  label: string;
  state: 'done' | 'current' | 'todo';
}

/** The numbered steps of a flow that runs on one page ("1 Analisar — 2 Análise ao vivo — 3 …"). */
export function Steps({ label, steps }: { label: string; steps: Step[] }) {
  return (
    <ol className={styles.steps} aria-label={label}>
      {steps.map((step, index) => (
        <li
          key={step.label}
          className={styles.step}
          data-state={step.state}
          aria-current={step.state === 'current' ? 'step' : undefined}
        >
          <span className={styles.number} aria-hidden="true">
            {step.state === 'done' ? <Icon name="check" size={12} /> : index + 1}
          </span>
          <span className={styles.text}>{step.label}</span>
        </li>
      ))}
    </ol>
  );
}
