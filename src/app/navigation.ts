import type { ComponentType } from 'react';
import { WorkflowPage } from '@/features/workflow/pages/WorkflowPage';
import { SettingsPage } from '@/features/settings/pages/SettingsPage';
import { WorkspacePage } from '@/features/workspace/pages/WorkspacePage';
import type { IconName } from '@/components/ui/Icon';
import type { TranslationKey } from '@/i18n';
import { AgentsScreen } from './AgentsScreen';
import { PersonalitiesScreen } from './PersonalitiesScreen';

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
