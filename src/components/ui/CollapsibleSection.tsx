import { useId, useState, type ReactNode } from 'react';
import { Icon } from './Icon';
import styles from './CollapsibleSection.module.css';

interface Props {
  title: string;
  /** Accessible name of the region; defaults to the title. */
  label?: string;
  /** What the section is in one word or two, on the right of its header (a state, a count). */
  aside?: ReactNode;
  defaultOpen?: boolean;
  children: ReactNode;
}

/** A titled block that folds down to its header, for the long panels of an inspection. */
export function CollapsibleSection({ title, label, aside, defaultOpen = true, children }: Props) {
  const [open, setOpen] = useState(defaultOpen);
  const bodyId = useId();
  return (
    <section className={styles.section} aria-label={label ?? title}>
      <div className={styles.head}>
        <h3 className={styles.title}>
          <button
            type="button"
            className={styles.toggle}
            aria-expanded={open}
            aria-controls={bodyId}
            onClick={() => {
              setOpen((current) => !current);
            }}
          >
            <Icon name="chevronDown" size={14} />
            {title}
          </button>
        </h3>
        {aside !== undefined && <span className={styles.aside}>{aside}</span>}
      </div>
      <div id={bodyId} className={styles.body} hidden={!open}>
        {children}
      </div>
    </section>
  );
}
