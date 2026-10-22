import { useEffect, useRef, type RefObject } from 'react';

/**
 * Closes a popover when the person clicks, touches or tabs outside it, or presses Escape.
 * Listens in the capture phase, so a screen that stops the event (a canvas, a drag area) cannot
 * keep a popover open.
 */
export function useDismiss(
  open: boolean,
  root: RefObject<HTMLElement | null>,
  onDismiss: () => void,
) {
  const dismiss = useRef(onDismiss);
  useEffect(() => {
    dismiss.current = onDismiss;
  });
  useEffect(() => {
    if (!open) return undefined;
    const outside = (event: Event) => {
      if (root.current && !root.current.contains(event.target as Node)) dismiss.current();
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') dismiss.current();
    };
    document.addEventListener('pointerdown', outside, true);
    document.addEventListener('focusin', outside, true);
    document.addEventListener('keydown', escape);
    return () => {
      document.removeEventListener('pointerdown', outside, true);
      document.removeEventListener('focusin', outside, true);
      document.removeEventListener('keydown', escape);
    };
  }, [open, root]);
}
