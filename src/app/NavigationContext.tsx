import { createContext, useContext } from 'react';

/** A hint for the target screen about what the user came to do. */
export type NavigationIntent =
  | { type: 'create-agent'; personalityId?: string }
  /** Show a workflow, and one of its runs when `executionId` is given. */
  | { type: 'open-workflow'; workflowId: string; executionId?: string }
  /** Show a spec, or the form to start a new one. */
  | { type: 'open-spec'; specId: string }
  | { type: 'new-spec' }
  /** Start a workflow from an approved spec (ADR 0035). */
  | { type: 'workflow-from-spec'; specId: string };

export interface Navigation {
  navigate: (screenId: string, intent?: NavigationIntent) => void;
  /** The intent the current screen was opened with, if any. */
  intent: NavigationIntent | undefined;
}

export const NavigationContext = createContext<Navigation | null>(null);

export function useNavigation(): Navigation {
  const navigation = useContext(NavigationContext);
  if (!navigation) throw new Error('useNavigation must be used inside the app');
  return navigation;
}
