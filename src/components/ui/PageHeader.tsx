import type { ReactNode } from 'react';
import { Icon } from './Icon';
import styles from './PageHeader.module.css';

interface Props {
  title: ReactNode;
  /** Next to the title: a state badge, a count, a mono path. */
  titleAside?: ReactNode;
  /** "← Projetos": where this page came from. */
  back?: { label: string; onClick: () => void };
  /** One line of mono metadata under the title ("Hoje US$ 1,24 · Semana US$ 3,72"). */
  meta?: ReactNode;
  /** One sentence that says what the page is for. */
  description?: ReactNode;
  /** The page's actions: secondary ones first, the one primary action last. */
  actions?: ReactNode;
  /** `page` (24px) for a screen; `section` (20px) for a page inside a flow. */
  size?: 'page' | 'section';
}

/**
 * The head of a page: where it came from, its title and what it is, and its actions on the
 * right. When the window is narrow the actions wrap below the title instead of overlapping it.
 */
export function PageHeader({
  title,
  titleAside,
  back,
  meta,
  description,
  actions,
  size = 'page',
}: Props) {
  return (
    <header className={styles.header} data-size={size}>
      <div className={styles.lead}>
        {back && (
          <button type="button" className={styles.back} onClick={back.onClick}>
            <Icon name="arrowLeft" size={14} />
            {back.label}
          </button>
        )}
        <div className={styles.titleRow}>
          <h1 className={styles.title}>{title}</h1>
          {titleAside}
        </div>
        {description !== undefined && <p className={styles.description}>{description}</p>}
        {meta !== undefined && <p className={styles.meta}>{meta}</p>}
      </div>
      {actions !== undefined && <div className={styles.actions}>{actions}</div>}
    </header>
  );
}
