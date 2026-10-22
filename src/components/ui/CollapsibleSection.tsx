import { useId, useState, type ReactNode } from 'react';
import { Icon } from './Icon';
import styles from './CollapsibleSection.module.css';

interface Props {
  title: string;
  /** Accessible name of the region; defaults to the title. */
  label?: string;
  /** What the section is in one word or two, on the right of its header (a state, a count). */
  aside?: ReactNode;
  /** One line that says what is inside while the section is closed. */
  summary?: ReactNode;
  defaultOpen?: boolean;
  /**
   * `card`: a bordered block of its own; `flat`: one of several sections split by hairlines (an
   * inspector); `disclosure`: a quiet "more about this" link that unfolds in place.
   */
  variant?: 'card' | 'flat' | 'disclosure';
  /** Controlled open state, for a parent that opens one section at a time. */
  open?: boolean;
  onToggle?: (open: boolean) => void;
  /** Draw the content only while open (long or heavy content, shown on demand). */
  lazy?: boolean;
  children: ReactNode;
}

/** A titled block that folds down to its header, for long panels and secondary detail. */
export function CollapsibleSection({
  title,
  label,
  aside,
  summary,
  defaultOpen = true,
  variant = 'card',
  open: controlled,
  onToggle,
  lazy = false,
  children,
}: Props) {
  const [own, setOwn] = useState(defaultOpen);
  const open = controlled ?? own;
  const bodyId = useId();
  return (
    <section
      className={styles.section}
      data-variant={variant}
      data-open={open}
      aria-label={label ?? title}
    >
      <div className={styles.head}>
        <h3 className={styles.title}>
          <button
            type="button"
            className={styles.toggle}
            aria-expanded={open}
            aria-controls={bodyId}
            onClick={() => {
              setOwn(!open);
              onToggle?.(!open);
            }}
          >
            {variant === 'disclosure' && (
              <Icon name="chevronDown" size={14} className={styles.chevron} />
            )}
            <span className={styles.titleText}>{title}</span>
            {!open && summary !== undefined && variant !== 'disclosure' && (
              <span className={styles.summary}>{summary}</span>
            )}
          </button>
        </h3>
        {aside !== undefined && <span className={styles.aside}>{aside}</span>}
        {variant !== 'disclosure' && (
          <Icon name="chevronDown" size={16} className={styles.chevron} />
        )}
      </div>
      <div id={bodyId} className={styles.body} hidden={!open}>
        {lazy && !open ? null : children}
      </div>
    </section>
  );
}
