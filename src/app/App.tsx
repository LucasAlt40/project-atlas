import { useMemo, useState } from 'react';
import { AppShell } from '@/components/layout/AppShell';
import { SidebarFooter } from '@/components/layout/SidebarFooter';
import { CatalogProvider } from '@/features/agents/hooks/useCatalog';
import { useAttentionNotifications } from '@/features/workflow/hooks/useAttentionNotifications';
import { ActionRequired } from '@/features/workflow/components/ActionRequired';
import { LanguageSwitch } from '@/features/settings/components/LanguageSwitch';
import { SettingsProvider } from '@/features/settings/hooks/SettingsProvider';
import { ActiveExecutions } from '@/features/workspace/components/ActiveExecutions';
import { WorkspaceSwitcher } from '@/features/workspace/components/WorkspaceSwitcher';
import { WorkspaceProvider, useWorkspace } from '@/features/workspace/hooks/WorkspaceProvider';
import { useT } from '@/i18n/I18nProvider';
import { NavigationContext, type Navigation, type NavigationIntent } from './NavigationContext';
import { DEFAULT_SCREEN_ID, SCREENS } from './navigation';

interface Location {
  screenId: string;
  intent: NavigationIntent | undefined;
}

function Shell() {
  const t = useT();
  const workspace = useWorkspace();
  const [location, setLocation] = useState<Location>({
    screenId: DEFAULT_SCREEN_ID,
    intent: undefined,
  });
  const active = SCREENS.find((screen) => screen.id === location.screenId) ?? SCREENS[0];
  const ActiveScreen = active.component;

  const navigation = useMemo<Navigation>(
    () => ({
      navigate: (screenId, intent) => {
        setLocation({ screenId, intent });
      },
      intent: location.intent,
    }),
    [location.intent],
  );

  // An agent asks while the app is behind: notify, and take the person to it on return.
  useAttentionNotifications((target) => {
    navigation.navigate('workflow', { type: 'open-workflow', ...target });
  });

  return (
    <NavigationContext.Provider value={navigation}>
      <AppShell
        items={SCREENS.map(({ id, labelKey, icon }) => ({ id, label: t(labelKey), icon }))}
        navLabel={t('nav.main')}
        collapseLabel={t('nav.collapse')}
        expandLabel={t('nav.expand')}
        activeId={active.id}
        onNavigate={(id) => {
          navigation.navigate(id);
        }}
        workspaceSwitcher={<WorkspaceSwitcher />}
        breadcrumb={{ parent: workspace.active?.name ?? '', current: t(active.labelKey) }}
        sidebarFooter={<SidebarFooter />}
        trailing={
          <>
            <ActionRequired />
            <ActiveExecutions />
            <LanguageSwitch />
          </>
        }
      >
        <ActiveScreen />
      </AppShell>
    </NavigationContext.Provider>
  );
}

/**
 * Providers sit above the screens. Global state (settings and language, the agent catalog) and
 * workspace state (workspaces, their conversations and runs) both outlive any screen: agents
 * keep running while another screen or another workspace is shown.
 */
export function App() {
  return (
    <SettingsProvider>
      <CatalogProvider>
        <WorkspaceProvider>
          <Shell />
        </WorkspaceProvider>
      </CatalogProvider>
    </SettingsProvider>
  );
}
