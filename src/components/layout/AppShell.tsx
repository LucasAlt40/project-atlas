import { useEffect, useState, type ReactNode } from 'react';
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
  /** Names the button that collapses and expands the sidebar. */
  collapseLabel: string;
  expandLabel: string;
  activeId: string;
  onNavigate: (id: string) => void;
  /** Shows and switches the current workspace; sits at the top of the sidebar. */
  workspaceSwitcher: ReactNode;
  /** Shown in the footer of the sidebar (for example the runtime summary). */
  sidebarFooter?: ReactNode;
  /** Shown at the end of the header (for example the language switch). */
  trailing?: ReactNode;
  /** Where the person is: shown at the start of the header. */
  breadcrumb?: { parent: string; current: string };
  children: ReactNode;
}

const COLLAPSED_KEY = 'atlas.sidebar.collapsed';

/** Below this width the window has no room for the sidebar, unless the user chose otherwise. */
const NARROW_QUERY = '(max-width: 899px)';

/** What the user chose for the sidebar, or `null` when they never did. */
function readChoice(): boolean | null {
  try {
    const stored = localStorage.getItem(COLLAPSED_KEY);
    return stored === null ? null : stored === '1';
  } catch {
    return null;
  }
}

/** Whether the window is narrow now, and when that changes. */
function useNarrowWindow(): boolean {
  const [narrow, setNarrow] = useState(
    () => typeof window.matchMedia === 'function' && window.matchMedia(NARROW_QUERY).matches,
  );
  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return undefined;
    const query = window.matchMedia(NARROW_QUERY);
    const update = () => {
      setNarrow(query.matches);
    };
    update();
    query.addEventListener('change', update);
    return () => {
      query.removeEventListener('change', update);
    };
  }, []);
  return narrow;
}

export function AppShell({
  items,
  navLabel,
  collapseLabel,
  expandLabel,
  activeId,
  onNavigate,
  workspaceSwitcher,
  sidebarFooter,
  trailing,
  breadcrumb,
  children,
}: Props) {
  const [choice, setChoice] = useState(readChoice);
  const narrow = useNarrowWindow();
  const collapsed = choice ?? narrow;
  const toggleLabel = collapsed ? expandLabel : collapseLabel;
  function toggle() {
    const next = !collapsed;
    setChoice(next);
    try {
      localStorage.setItem(COLLAPSED_KEY, next ? '1' : '0');
    } catch {
      // The preference simply is not remembered.
    }
  }
  return (
    <div className={styles.shell} data-collapsed={collapsed}>
      <aside className={styles.rail}>
        <div className={styles.brandBar}>
          <span className={styles.brand}>
            <AtlasLogo size={28} />
            <span className={styles.brandName}>Atlas</span>
          </span>
        </div>
        <div className={styles.switcherSlot}>{workspaceSwitcher}</div>
        <nav aria-label={navLabel}>
          <ul className={styles.nav}>
            {items.map(({ id, label, icon }) => (
              <li key={id}>
                <button
                  type="button"
                  className={styles.link}
                  aria-current={id === activeId ? 'page' : undefined}
                  title={collapsed ? label : undefined}
                  aria-label={collapsed ? label : undefined}
                  onClick={() => {
                    onNavigate(id);
                  }}
                >
                  {icon && <Icon name={icon} />}
                  <span className={styles.linkLabel}>{label}</span>
                </button>
              </li>
            ))}
          </ul>
        </nav>
        {sidebarFooter && <div className={styles.railFooter}>{sidebarFooter}</div>}
      </aside>
      <div className={styles.column}>
        <header className={styles.header}>
          <button
            type="button"
            className={styles.toggle}
            aria-label={toggleLabel}
            aria-expanded={!collapsed}
            title={toggleLabel}
            onClick={toggle}
          >
            <Icon name="panelLeft" size={18} />
          </button>
          <div className={styles.crumb}>
            {breadcrumb && (
              <>
                <span className={styles.crumbParent}>{breadcrumb.parent}</span>
                <span className={styles.crumbSep}>/</span>
                <strong className={styles.crumbCurrent}>{breadcrumb.current}</strong>
              </>
            )}
          </div>
          <div className={styles.trailing}>{trailing}</div>
        </header>
        <main className={styles.main}>{children}</main>
      </div>
    </div>
  );
}
