import { createContext, useContext, useEffect } from 'react';

/** Sets the levels below the screen in the header's breadcrumb ("… / Projetos / Harness"). */
export const BreadcrumbContext = createContext<((trail: string[]) => void) | null>(null);

/**
 * Declares where the person is inside a screen: the levels after the screen's name, in order.
 * A view that is a level of its own (a page in a flow, an open drawer) calls this while shown.
 */
export function useBreadcrumb(trail: readonly (string | null | undefined | false)[]): void {
  const set = useContext(BreadcrumbContext);
  const levels = trail.filter((level): level is string => typeof level === 'string' && !!level);
  const key = levels.join('\u0000');
  useEffect(() => {
    if (!set) return undefined;
    set(key ? key.split('\u0000') : []);
    return () => {
      set([]);
    };
  }, [set, key]);
}
