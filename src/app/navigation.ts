import { lazy, type ComponentType } from 'react';
import { WorkspacePage } from '@/features/workspace/pages/WorkspacePage';
import type { IconName } from '@/components/ui/Icon';
import type { TranslationKey } from '@/i18n';

// Only the first screen is in the main bundle; the others load when first opened (the workflow
// canvas alone brings in a graph library). A screen is unmounted when it is left, so a lazy
// one costs nothing after its first load.
const WorkflowPage = lazy(() =>
  import('@/features/workflow/pages/WorkflowPage').then((m) => ({ default: m.WorkflowPage })),
);
const SettingsPage = lazy(() =>
  import('@/features/settings/pages/SettingsPage').then((m) => ({ default: m.SettingsPage })),
);
const AgentsScreen = lazy(() =>
  import('./AgentsScreen').then((m) => ({ default: m.AgentsScreen })),
);
const PersonalitiesScreen = lazy(() =>
  import('./PersonalitiesScreen').then((m) => ({ default: m.PersonalitiesScreen })),
);

export interface Screen {
  id: string;
  labelKey: TranslationKey;
  icon: IconName;
  component: ComponentType;
}

/**
 * The screens reachable from the navigation. Adding a screen (Projects, Workflows,
 * Executions…) means adding an entry here. A router can replace this once screens need URLs or
 * nested routes. The Workspace is the primary experience.
 */
export const SCREENS: readonly [Screen, ...Screen[]] = [
  { id: 'workspace', labelKey: 'nav.workspace', icon: 'workspace', component: WorkspacePage },
  { id: 'workflow', labelKey: 'nav.workflow', icon: 'workflow', component: WorkflowPage },
  { id: 'agents', labelKey: 'nav.agents', icon: 'agents', component: AgentsScreen },
  {
    id: 'personalities',
    labelKey: 'nav.personalities',
    icon: 'personalities',
    component: PersonalitiesScreen,
  },
  { id: 'settings', labelKey: 'nav.settings', icon: 'settings', component: SettingsPage },
];

export const DEFAULT_SCREEN_ID = 'workspace';
