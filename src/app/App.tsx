import { Suspense, useEffect, useMemo, useState } from 'react';
import { BreadcrumbContext } from './BreadcrumbContext';
import { ErrorBoundary } from '@/components/ui/ErrorBoundary';
import { AppShell } from '@/components/layout/AppShell';
import { SidebarFooter } from '@/components/layout/SidebarFooter';
import { CatalogProvider } from '@/features/agents/hooks/useCatalog';
import { useAttentionNotifications } from '@/features/workflow/hooks/useAttentionNotifications';
import { ActionRequired } from '@/features/workflow/components/ActionRequired';
import { MobileAccessButton } from '@/features/remote/MobileAccessButton';
import { LanguageSwitch } from '@/features/settings/components/LanguageSwitch';
import { setPresenceScreen } from '@/features/settings/services/settingsService';
import { SettingsProvider } from '@/features/settings/hooks/SettingsProvider';
import { ActiveExecutions } from '@/features/workspace/components/ActiveExecutions';
import { WorkspaceSwitcher } from '@/features/workspace/components/WorkspaceSwitcher';
import { WorkspaceProvider, useWorkspace } from '@/features/workspace/hooks/WorkspaceProvider';
import { useT } from '@/i18n/I18nProvider';
import type { PresenceScreenDto } from '@/lib/tauri/commands';
import { NavigationContext, type Navigation, type NavigationIntent } from './NavigationContext';
import { DEFAULT_SCREEN_ID, SCREENS } from './navigation';

interface Location {
  screenId: string;
  intent: NavigationIntent | undefined;
}

/** The screens that have a line on the Discord profile. */
const PRESENCE_SCREEN: Record<string, PresenceScreenDto> = {
  workspace: 'workspace',
  workflow: 'workflow',
  agents: 'agents',
  personalities: 'personalities',
  specs: 'specs',
  mcp: 'mcp_hub',
  settings: 'settings',
};

function Shell() {
  const t = useT();
  const workspace = useWorkspace();
  const [location, setLocation] = useState<Location>({
    screenId: DEFAULT_SCREEN_ID,
    intent: undefined,
  });
  // The levels below the screen ("Projetos / Harness"), declared by the view that is shown.
  const [trail, setTrail] = useState<string[]>([]);
  const screens = SCREENS;
  const active = screens.find((screen) => screen.id === location.screenId) ?? SCREENS[0];
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

  // The profile on Discord says which part of Atlas is open (only its name, never its content).
  useEffect(() => {
    const screen = PRESENCE_SCREEN[location.screenId];
    if (screen) Promise.resolve(setPresenceScreen(screen)).catch(() => undefined);
  }, [location.screenId]);

  // An agent asks while the app is behind: notify, and take the person to it on return.
  useAttentionNotifications((target) => {
    navigation.navigate('workflow', { type: 'open-workflow', ...target });
  });

  return (
    <NavigationContext.Provider value={navigation}>
      <BreadcrumbContext.Provider value={setTrail}>
        <AppShell
          items={screens.map(({ id, labelKey, icon }) => ({ id, label: t(labelKey), icon }))}
          navLabel={t('nav.main')}
          collapseLabel={t('nav.collapse')}
          expandLabel={t('nav.expand')}
          activeId={active.id}
          onNavigate={(id) => {
            navigation.navigate(id);
          }}
          workspaceSwitcher={<WorkspaceSwitcher />}
          breadcrumb={[
            ...(workspace.active ? [workspace.active.name] : []),
            t(active.labelKey),
            ...trail,
          ]}
          breadcrumbLabel={t('nav.breadcrumb')}
          sidebarFooter={<SidebarFooter />}
          trailing={
            <>
              <ActionRequired />
              <ActiveExecutions />
              <MobileAccessButton />
              <LanguageSwitch />
            </>
          }
        >
          <ErrorBoundary resetKey={active.id}>
            <Suspense fallback={null}>
              <ActiveScreen />
            </Suspense>
          </ErrorBoundary>
        </AppShell>
      </BreadcrumbContext.Provider>
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
