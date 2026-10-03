import type { ComponentType } from 'react';
import { SettingsPage } from '@/features/settings/pages/SettingsPage';
import { WorkspacePage } from '@/features/workspace/pages/WorkspacePage';
import type { TranslationKey } from '@/i18n';
import { AgentsScreen } from './AgentsScreen';
import { PersonalitiesScreen } from './PersonalitiesScreen';

export interface Screen {
  id: string;
  labelKey: TranslationKey;
  component: ComponentType;
}

/**
 * The screens reachable from the navigation. Adding a screen (Projects, Workflows,
 * Executions…) means adding an entry here. A router can replace this once screens need URLs or
 * nested routes. The Workspace is the primary experience.
 */
export const SCREENS: readonly [Screen, ...Screen[]] = [
  { id: 'workspace', labelKey: 'nav.workspace', component: WorkspacePage },
  { id: 'agents', labelKey: 'nav.agents', component: AgentsScreen },
  { id: 'personalities', labelKey: 'nav.personalities', component: PersonalitiesScreen },
  { id: 'settings', labelKey: 'nav.settings', component: SettingsPage },
];

export const DEFAULT_SCREEN_ID = 'workspace';
