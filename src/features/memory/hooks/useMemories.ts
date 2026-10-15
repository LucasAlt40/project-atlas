import { useCallback, useEffect, useState } from 'react';
import type { MemoryDto, MemoryStatusDto } from '@/lib/tauri/commands';
import {
  approveMemory,
  createMemory,
  deleteAutomaticMemories,
  deleteMemory,
  editMemory,
  listMemories,
  promoteMemory,
  setMemoryStatus,
  type NewMemoryInput,
} from '../services/memoryService';

/** A mocked service in a test may answer nothing; the list is then empty. */
const orNone = (memories: MemoryDto[] | undefined): MemoryDto[] => memories ?? [];

export type MemoriesState =
  | { status: 'loading' }
  | { status: 'error'; error: unknown }
  | { status: 'ready'; memories: MemoryDto[] };

/** The notes Atlas keeps, and what a person may do with them. Every action updates the list. */
export function useMemories() {
  const [state, setState] = useState<MemoriesState>({ status: 'loading' });

  const load = useCallback(async () => {
    try {
      setState({ status: 'ready', memories: orNone(await listMemories()) });
    } catch (error) {
      setState({ status: 'error', error });
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    Promise.resolve(listMemories())
      .then((memories) => {
        if (!cancelled) setState({ status: 'ready', memories: orNone(memories) });
      })
      .catch((error: unknown) => {
        if (!cancelled) setState({ status: 'error', error });
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const replace = useCallback((changed: MemoryDto) => {
    setState((current) =>
      current.status === 'ready'
        ? {
            status: 'ready',
            memories: current.memories.map((m) => (m.id === changed.id ? changed : m)),
          }
        : current,
    );
  }, []);

  const create = useCallback(async (input: NewMemoryInput) => {
    const made = await createMemory(input);
    setState((current) =>
      current.status === 'ready'
        ? { status: 'ready', memories: [...current.memories, made] }
        : current,
    );
  }, []);

  const edit = useCallback(
    async (id: string, title: string, content: string, topic: string | null) => {
      replace(await editMemory(id, title, content, topic));
    },
    [replace],
  );

  const approve = useCallback(
    async (id: string) => {
      replace(await approveMemory(id));
    },
    [replace],
  );

  const promote = useCallback(
    async (id: string) => {
      replace(await promoteMemory(id));
    },
    [replace],
  );

  const setStatus = useCallback(
    async (id: string, status: MemoryStatusDto) => {
      replace(await setMemoryStatus(id, status));
    },
    [replace],
  );

  const remove = useCallback(async (id: string) => {
    await deleteMemory(id);
    setState((current) =>
      current.status === 'ready'
        ? { status: 'ready', memories: current.memories.filter((m) => m.id !== id) }
        : current,
    );
  }, []);

  const clearAutomatic = useCallback(async () => {
    await deleteAutomaticMemories();
    await load();
  }, [load]);

  return { state, create, edit, approve, promote, setStatus, remove, clearAutomatic };
}
