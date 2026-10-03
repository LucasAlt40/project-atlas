import { createContext, createElement, useCallback, useContext, useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import {
  createAgent,
  createPersonality,
  deleteAgent,
  deletePersonality,
  listAgents,
  listPersonalities,
  listRuntimes,
  restoreDefaultPersonalities,
  updateAgent,
  updatePersonality,
} from '../services/catalogService';
import type {
  Agent,
  CreateAgentInput,
  CreatePersonalityInput,
  Personality,
  RuntimeStatus,
} from '../types';

export type CatalogState =
  | { status: 'loading' }
  | { status: 'error'; error: unknown }
  | { status: 'ready'; personalities: Personality[]; agents: Agent[] };

type CatalogReady = Extract<CatalogState, { status: 'ready' }>;

export type RuntimesState =
  | { status: 'loading' }
  | { status: 'error'; error: unknown }
  | { status: 'ready'; runtimes: RuntimeStatus[] };

/**
 * What the user can choose from, shared by every workspace: personalities, saved agents, and
 * the AI runtimes detected on this machine. Runtime detection is slow (it starts local
 * processes), so it loads separately and never blocks the rest.
 */
function useCatalogState() {
  const [catalog, setCatalog] = useState<CatalogState>({ status: 'loading' });
  const [runtimes, setRuntimes] = useState<RuntimesState>({ status: 'loading' });

  const loadRuntimes = useCallback((isCancelled: () => boolean = () => false) => {
    listRuntimes()
      .then((list) => {
        if (!isCancelled()) setRuntimes({ status: 'ready', runtimes: list });
      })
      .catch((error: unknown) => {
        if (!isCancelled()) setRuntimes({ status: 'error', error });
      });
  }, []);

  const refreshRuntimes = useCallback(() => {
    setRuntimes({ status: 'loading' });
    loadRuntimes();
  }, [loadRuntimes]);

  useEffect(() => {
    let cancelled = false;
    Promise.all([listPersonalities(), listAgents()])
      .then(([personalities, agents]) => {
        if (!cancelled) setCatalog({ status: 'ready', personalities, agents });
      })
      .catch((error: unknown) => {
        if (!cancelled) setCatalog({ status: 'error', error });
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    loadRuntimes(() => cancelled);
    return () => {
      cancelled = true;
    };
  }, [loadRuntimes]);

  const whenReady = useCallback((change: (ready: CatalogReady) => CatalogReady) => {
    setCatalog((c) => (c.status === 'ready' ? change(c) : c));
  }, []);

  const addPersonality = useCallback(
    async (input: CreatePersonalityInput) => {
      const created = await createPersonality(input);
      whenReady((c) => ({ ...c, personalities: [...c.personalities, created] }));
      return created;
    },
    [whenReady],
  );

  const editPersonality = useCallback(
    async (id: string, input: CreatePersonalityInput) => {
      const updated = await updatePersonality(id, input);
      whenReady((c) => ({
        ...c,
        personalities: c.personalities.map((p) => (p.id === id ? updated : p)),
      }));
      return updated;
    },
    [whenReady],
  );

  const removePersonality = useCallback(
    async (id: string) => {
      await deletePersonality(id);
      whenReady((c) => ({ ...c, personalities: c.personalities.filter((p) => p.id !== id) }));
    },
    [whenReady],
  );

  const restorePersonalities = useCallback(async () => {
    const personalities = await restoreDefaultPersonalities();
    whenReady((c) => ({ ...c, personalities }));
  }, [whenReady]);

  const addAgent = useCallback(
    async (input: CreateAgentInput) => {
      const created = await createAgent(input);
      whenReady((c) => ({ ...c, agents: [...c.agents, created] }));
      return created;
    },
    [whenReady],
  );

  const editAgent = useCallback(
    async (id: string, input: CreateAgentInput) => {
      const updated = await updateAgent(id, input);
      whenReady((c) => ({ ...c, agents: c.agents.map((a) => (a.id === id ? updated : a)) }));
      return updated;
    },
    [whenReady],
  );

  const removeAgent = useCallback(
    async (id: string) => {
      await deleteAgent(id);
      whenReady((c) => ({ ...c, agents: c.agents.filter((a) => a.id !== id) }));
    },
    [whenReady],
  );

  return {
    catalog,
    runtimes,
    refreshRuntimes,
    addPersonality,
    editPersonality,
    removePersonality,
    restorePersonalities,
    addAgent,
    editAgent,
    removeAgent,
  };
}

export type Catalog = ReturnType<typeof useCatalogState>;

const CatalogContext = createContext<Catalog | null>(null);

/**
 * Loads the catalog once and shares it, so every screen sees the same agents (an agent
 * created on the Agents screen is immediately known to the workspace).
 */
export function CatalogProvider({ children }: { children: ReactNode }) {
  return createElement(CatalogContext.Provider, { value: useCatalogState() }, children);
}

export function useCatalog(): Catalog {
  const catalog = useContext(CatalogContext);
  if (!catalog) throw new Error('useCatalog must be used inside <CatalogProvider>');
  return catalog;
}
