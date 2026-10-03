import { useAppInfo } from '@/features/home/hooks/useAppInfo';
import styles from './AppShell.module.css';

/** The app name and version, anchored at the bottom of the sidebar. */
export function SidebarFooter() {
  const state = useAppInfo();
  if (state.status !== 'ready') return null;
  return (
    <>
      <span className={styles.footerName}>{state.info.name}</span>
      <span className={styles.footerMeta}>
        v{state.info.version} · {state.info.platform}
      </span>
    </>
  );
}
