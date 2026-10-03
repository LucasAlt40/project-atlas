import { createContext, useContext } from 'react';

/** A hint for the target screen about what the user came to do. */
export interface NavigationIntent {
  type: 'create-agent';
  personalityId?: string;
}

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
