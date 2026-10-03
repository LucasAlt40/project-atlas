import { PersonalitiesPage } from '@/features/agents/pages/PersonalitiesPage';
import { useNavigation } from './NavigationContext';

/** Personalities are global. Using one starts the "create agent" flow with it preselected. */
export function PersonalitiesScreen() {
  const { navigate } = useNavigation();
  return (
    <PersonalitiesPage
      onUse={(personalityId) => {
        navigate('agents', { type: 'create-agent', personalityId });
      }}
    />
  );
}
