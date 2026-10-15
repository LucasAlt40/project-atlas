import {
  invokeCommand,
  type MemoryDto,
  type MemoryKindDto,
  type MemoryScopeDto,
  type MemoryStatusDto,
} from '@/lib/tauri/commands';

/** Notes of earlier work (ADR 0037). Atlas keeps them; a note is a hint, never an instruction. */

export function listMemories(): Promise<MemoryDto[]> {
  return invokeCommand('list_memories');
}

export interface NewMemoryInput {
  scope: MemoryScopeDto;
  /** The workspace or agent id; none for the user's own. */
  owner: string | null;
  kind: MemoryKindDto;
  title: string;
  content: string;
  topic?: string | null;
}

export function createMemory(input: NewMemoryInput): Promise<MemoryDto> {
  return invokeCommand('create_memory', { ...input });
}

export function editMemory(
  id: string,
  title: string,
  content: string,
  topic: string | null,
): Promise<MemoryDto> {
  return invokeCommand('edit_memory', { id, title, content, topic });
}

export function approveMemory(id: string): Promise<MemoryDto> {
  return invokeCommand('approve_memory', { id });
}

export function promoteMemory(id: string): Promise<MemoryDto> {
  return invokeCommand('promote_memory', { id });
}

export function setMemoryStatus(id: string, status: MemoryStatusDto): Promise<MemoryDto> {
  return invokeCommand('set_memory_status', { id, status });
}

export function deleteMemory(id: string): Promise<null> {
  return invokeCommand('delete_memory', { id });
}

/** Deletes the automatic notes nobody took as theirs; the number deleted. */
export function deleteAutomaticMemories(): Promise<number> {
  return invokeCommand('delete_automatic_memories');
}
