import type { ReactNode } from 'react';
import { AtlasLogo } from '@/components/brand/AtlasLogo';
import { Icon, type IconName } from '@/components/ui/Icon';
import styles from './AppShell.module.css';

export interface NavItem {
  id: string;
  label: string;
  icon?: IconName;
}

interface Props {
  items: readonly NavItem[];
  navLabel: string;
  activeId: string;
  onNavigate: (id: string) => void;
  /** Shows and switches the current workspace; always visible in the header. */
  workspaceSwitcher: ReactNode;
  /** Shown in the footer of the sidebar (for example the runtime summary). */
  sidebarFooter?: ReactNode;
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
  sidebarFooter,
  trailing,
  children,
}: Props) {
  return (
    <div className={styles.shell}>
      <header className={styles.header}>
        <span className={styles.brand}>
          <AtlasLogo size={26} />
          <span className={styles.brandName}>Atlas</span>
        </span>
        {workspaceSwitcher}
        <div className={styles.trailing}>{trailing}</div>
      </header>
      <div className={styles.body}>
        <aside className={styles.rail}>
          <nav aria-label={navLabel}>
            <ul className={styles.nav}>
              {items.map(({ id, label, icon }) => (
                <li key={id}>
                  <button
                    type="button"
                    className={styles.link}
                    aria-current={id === activeId ? 'page' : undefined}
                    onClick={() => {
                      onNavigate(id);
                    }}
                  >
                    {icon && <Icon name={icon} />}
                    {label}
                  </button>
                </li>
              ))}
            </ul>
          </nav>
          {sidebarFooter && <div className={styles.railFooter}>{sidebarFooter}</div>}
        </aside>
        <main className={styles.main}>{children}</main>
      </div>
    </div>
  );
}
