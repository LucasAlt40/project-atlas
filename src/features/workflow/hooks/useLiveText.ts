import { useEffect, useState } from 'react';

export type Text<T> =
  { status: 'loading' } | { status: 'error'; error: unknown } | { status: 'ready'; data: T };

interface Entry<T> {
  key: string;
  /** What the same subject showed before: kept on screen while it is read again. */
  subject: string;
  value: Text<T>;
}

/**
 * Reads something for a subject (a file) whenever `key` changes, and says only what belongs to
 * the current key. While the same subject is being read again, the previous result stays (the
 * viewer does not blink or lose its scroll position); a different subject starts from loading.
 * `key` must contain the subject: that is how a late answer for another file is told apart.
 */
export function useLiveText<T>(
  subject: string,
  key: string,
  load: (() => Promise<T>) | null,
): { text: Text<T>; refreshing: boolean } {
  const [entry, setEntry] = useState<Entry<T> | null>(null);
  const enabled = load !== null;

  useEffect(() => {
    if (!load) return;
    let cancelled = false;
    load()
      .then((data) => {
        if (!cancelled) setEntry({ key, subject, value: { status: 'ready', data } });
      })
      .catch((error: unknown) => {
        if (!cancelled) setEntry({ key, subject, value: { status: 'error', error } });
      });
    return () => {
      cancelled = true;
    };
    // `load` is made fresh each render; `key` says when it would answer differently.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, enabled]);

  if (!enabled) return { text: { status: 'loading' }, refreshing: false };
  if (entry?.key === key) return { text: entry.value, refreshing: false };
  if (entry?.subject === subject && entry.value.status === 'ready') {
    return { text: entry.value, refreshing: true };
  }
  return { text: { status: 'loading' }, refreshing: false };
}
