import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import { useCatalog } from '@/features/agents/hooks/useCatalog';
import { useSettings } from '@/features/settings/hooks/SettingsProvider';
import { canAddAgent } from '../model/grid';
import {
  addAgentToWorkspace,
  createWorkspace,
  deleteWorkspace,
  listWorkspaces,
  removeAgentFromWorkspace,
  updateWorkspace,
} from '../services/workspaceService';
import type { Workspace, WorkspaceInput } from '../types';
import { useAgentConversations } from './useAgentConversations';

/** Asks an agent's card to show a view, e.g. when its run is picked from the global list. */
export interface FocusRequest {
  workspaceId: string;
  agentId: string;
  tab: 'chat' | 'terminal' | 'details';
}

export type WorkspacesState =
  | { status: 'loading' }
  | { status: 'error'; error: unknown }
  | { status: 'ready'; workspaces: Workspace[] };

/** Why an agent could not be placed: the UI's capacity limit, or an error from the core. */
export type AddResult =
  { ok: true } | { ok: false; capacity: true } | { ok: false; capacity: false; error: unknown };

function useWorkspaceState() {
  const settings = useSettings();
  const { catalog } = useCatalog();
  const conversations = useAgentConversations();
  const [state, setState] = useState<WorkspacesState>({ status: 'loading' });
  const [focus, setFocus] = useState<FocusRequest | null>(null);
  const { selectedWorkspaceId, rememberWorkspace } = settings;

  const reload = useCallback(() => {
    listWorkspaces()
      .then((workspaces) => {
        setState({ status: 'ready', workspaces });
      })
      .catch((error: unknown) => {
        setState((current) => (current.status === 'ready' ? current : { status: 'error', error }));
      });
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  const workspaces = useMemo(() => (state.status === 'ready' ? state.workspaces : []), [state]);
  // The one that was open last, or the first if that one is gone.
  const active = workspaces.find((w) => w.id === selectedWorkspaceId) ?? workspaces[0];

  // Keep the remembered selection in step with what is actually shown.
  const activeId = active?.id ?? null;
  useEffect(() => {
    if (state.status === 'ready' && activeId !== selectedWorkspaceId) {
      void rememberWorkspace(activeId);
    }
  }, [state.status, activeId, selectedWorkspaceId, rememberWorkspace]);

  // An agent deleted elsewhere has already left the workspaces in the core: re-read them.
  useEffect(() => {
    if (catalog.status !== 'ready' || state.status !== 'ready') return;
    const known = new Set(catalog.agents.map((agent) => agent.id));
    const stale = state.workspaces.some((w) =>
      w.layout.agentPlacements.some((p) => !known.has(p.agentId)),
    );
    if (stale) reload();
  }, [catalog, state, reload]);

  const replace = useCallback((updated: Workspace) => {
    setState((current) =>
      current.status === 'ready'
        ? {
            status: 'ready',
            workspaces: current.workspaces.map((w) => (w.id === updated.id ? updated : w)),
          }
        : current,
    );
  }, []);

  const select = useCallback((id: string) => rememberWorkspace(id), [rememberWorkspace]);

  const create = useCallback(
    async (input: WorkspaceInput) => {
      const created = await createWorkspace(input);
      setState((current) => ({
        status: 'ready',
        workspaces: [...(current.status === 'ready' ? current.workspaces : []), created],
      }));
      await rememberWorkspace(created.id);
      return created;
    },
    [rememberWorkspace],
  );

  const update = useCallback(
    async (id: string, input: WorkspaceInput) => {
      const updated = await updateWorkspace(id, input);
      replace(updated);
      return updated;
    },
    [replace],
  );

  const remove = useCallback(async (id: string) => {
    const remaining = await deleteWorkspace(id);
    setState({ status: 'ready', workspaces: remaining });
  }, []);

  const addAgent = useCallback(
    async (agentId: string): Promise<AddResult> => {
      if (!active) return { ok: false, capacity: false, error: undefined };
      if (active.layout.agentPlacements.some((p) => p.agentId === agentId)) return { ok: true };
      if (!canAddAgent(active)) return { ok: false, capacity: true };
      try {
        replace(await addAgentToWorkspace(active.id, agentId));
        return { ok: true };
      } catch (error) {
        return { ok: false, capacity: false, error };
      }
    },
    [active, replace],
  );

  const removeAgent = useCallback(
    async (agentId: string) => {
      if (!active) return;
      replace(await removeAgentFromWorkspace(active.id, agentId));
    },
    [active, replace],
  );

  // Shows the agent's card: its workspace (running agents elsewhere are untouched) and a view.
  const focusAgent = useCallback(
    async (request: FocusRequest) => {
      await rememberWorkspace(request.workspaceId);
      setFocus(request);
    },
    [rememberWorkspace],
  );
  const clearFocus = useCallback(() => {
    setFocus(null);
  }, []);

  const { send } = conversations;
  const sendToAgent = useCallback(
    (agentId: string, content: string) => (active ? send(active.id, agentId, content) : undefined),
    [send, active],
  );

  return {
    state,
    workspaces,
    active,
    select,
    create,
    update,
    remove,
    addAgent,
    removeAgent,
    conversations: conversations.state,
    usageVersion: conversations.usageVersion,
    terminals: conversations.hub,
    history: conversations.history,
    worktrees: conversations.worktrees,
    mergeExecution: conversations.merge,
    focus,
    focusAgent,
    clearFocus,
    sendToAgent,
  };
}

export type WorkspaceContextValue = ReturnType<typeof useWorkspaceState>;

const WorkspaceContext = createContext<WorkspaceContextValue | null>(null);

/**
 * Workspaces and their conversations. Lives above the screens, so an agent keeps running (and
 * its activity keeps updating) while another screen or another workspace is shown.
 */
export function WorkspaceProvider({ children }: { children: ReactNode }) {
  const value = useWorkspaceState();
  return <WorkspaceContext.Provider value={value}>{children}</WorkspaceContext.Provider>;
}

/** Like [`useWorkspace`], for components that also render without a provider (and then know nothing). */
export function useOptionalWorkspace(): WorkspaceContextValue | null {
  return useContext(WorkspaceContext);
}

export function useWorkspace(): WorkspaceContextValue {
  const value = useContext(WorkspaceContext);
  if (!value) throw new Error('useWorkspace must be used inside <WorkspaceProvider>');
  return value;
}
