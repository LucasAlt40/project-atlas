const KEY = 'atlas.preferredIde';

/** Remembers the editor the person chose, so the next time it is the one that opens. */
export function rememberIde(id: string): void {
  try {
    localStorage.setItem(KEY, id);
  } catch {
    // Private windows and blocked storage: the first editor found is used instead.
  }
}

/** The editor the person last chose, if it is still installed; else the first one found. */
export function preferredIde<T extends { id: string }>(ides: readonly T[]): T | undefined {
  let saved: string | null = null;
  try {
    saved = localStorage.getItem(KEY);
  } catch {
    saved = null;
  }
  return ides.find((ide) => ide.id === saved) ?? ides[0];
}
