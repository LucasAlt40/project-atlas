import type { ReactNode } from 'react';
import styles from './AppShell.module.css';

export interface NavItem {
  id: string;
  label: string;
}

interface Props {
  items: readonly NavItem[];
  navLabel: string;
  activeId: string;
  onNavigate: (id: string) => void;
  /** Shows and switches the current workspace; always visible in the header. */
  workspaceSwitcher: ReactNode;
  /** Shown at the end of the header (for example the language switch). */
  trailing?: ReactNode;
  children: ReactNode;
}

export function AppShell({
  items,
  navLabel,
  activeId,
  onNavigate,
  workspaceSwitcher,
  trailing,
  children,
}: Props) {
  return (
    <div className={styles.shell}>
      <header className={styles.header}>
        <span className={styles.brand}>Atlas</span>
        {workspaceSwitcher}
        <nav aria-label={navLabel}>
          <ul className={styles.nav}>
            {items.map(({ id, label }) => (
              <li key={id}>
                <button
                  type="button"
                  className={styles.link}
                  aria-current={id === activeId ? 'page' : undefined}
                  onClick={() => {
                    onNavigate(id);
                  }}
                >
                  {label}
                </button>
              </li>
            ))}
          </ul>
        </nav>
        <div className={styles.trailing}>{trailing}</div>
      </header>
      <main className={styles.main}>{children}</main>
    </div>
  );
}
