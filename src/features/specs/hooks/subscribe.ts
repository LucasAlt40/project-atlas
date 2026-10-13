/**
 * Opens a subscription now and gives back the function that closes it, which can be called before
 * the subscription is even ready (the component left): it is closed as soon as it is.
 */
export function subscribe(open: () => Promise<(() => void) | undefined>): () => void {
  let cancelled = false;
  let close: (() => void) | undefined;
  // A subscription that cannot open only means no live updates: the screen still works.
  void Promise.resolve(open())
    .then((unlisten) => {
      if (cancelled) unlisten?.();
      else close = unlisten;
    })
    .catch(() => undefined);
  return () => {
    cancelled = true;
    close?.();
  };
}
