import { useCallback, useState } from 'react';

const key = (specId: string) => `atlas.specs.answers.${specId}`;

function read(specId: string): Record<string, string> {
  try {
    const raw = localStorage.getItem(key(specId));
    const parsed: unknown = raw ? JSON.parse(raw) : {};
    if (typeof parsed !== 'object' || parsed === null) return {};
    return Object.fromEntries(
      Object.entries(parsed).filter(
        (entry): entry is [string, string] => typeof entry[1] === 'string',
      ),
    );
  } catch {
    return {};
  }
}

function write(specId: string, drafts: Record<string, string>) {
  try {
    if (Object.keys(drafts).length === 0) localStorage.removeItem(key(specId));
    else localStorage.setItem(key(specId), JSON.stringify(drafts));
  } catch {
    // Without storage the answers live as long as the screen does.
  }
}

/**
 * Answers the person has written but not sent yet, by finding. They survive leaving the screen,
 * so that answering is not a race against opening another document.
 */
export function useDrafts(specId: string) {
  const [drafts, setDrafts] = useState<Record<string, string>>(() => read(specId));
  const change = useCallback(
    (update: (current: Record<string, string>) => Record<string, string>) => {
      setDrafts((current) => {
        const next = update(current);
        write(specId, next);
        return next;
      });
    },
    [specId],
  );
  const set = useCallback(
    (id: string, text: string) => {
      change((current) => {
        const rest = Object.fromEntries(Object.entries(current).filter(([key]) => key !== id));
        return text === '' ? rest : { ...rest, [id]: text };
      });
    },
    [change],
  );
  const clear = useCallback(
    (ids: string[]) => {
      change((current) =>
        Object.fromEntries(Object.entries(current).filter(([id]) => !ids.includes(id))),
      );
    },
    [change],
  );
  return { drafts, set, clear };
}
