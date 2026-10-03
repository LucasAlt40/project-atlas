import '@xterm/xterm/css/xterm.css';
import type { Terminal } from '@xterm/xterm';
import type { SearchAddon } from '@xterm/addon-search';
import { useEffect, useImperativeHandle, useRef, type Ref } from 'react';
import type { TerminalHub } from '../model/terminalHub';
import styles from './Terminal.module.css';

/** What the toolbar can ask of the terminal view. */
export interface TerminalViewHandle {
  /** The selection, or everything when nothing is selected. */
  copy: () => Promise<void>;
  clear: () => void;
  find: (query: string, direction: 'next' | 'previous') => boolean;
}

interface Props {
  executionId: string;
  hub: TerminalHub;
  /** Accessible name of the terminal region. */
  label: string;
  /** No manual input: the terminal only shows what the process writes. */
  readOnly: boolean;
  /** Follow the output as it grows; when off the view stays where the user scrolled. */
  autoScroll: boolean;
  /** Ctrl+C in the terminal (with nothing selected). Works whatever `readOnly` says. */
  onInterrupt: () => void;
  /** Manual input, only when the runtime reads it. */
  onInput: ((data: string) => void) | undefined;
  /** The view changed size, in characters. */
  onResize: ((cols: number, rows: number) => void) | undefined;
  ref?: Ref<TerminalViewHandle>;
}

const RESIZE_DEBOUNCE_MS = 150;
/** Smaller than this is a layout that has not settled (or is hidden), not a real size. */
const MIN_COLS = 20;
const MIN_ROWS = 3;
const SCROLLBACK_LINES = 10_000;

function cssColor(element: HTMLElement, name: string, fallback: string): string {
  return getComputedStyle(element).getPropertyValue(name).trim() || fallback;
}

/**
 * The terminal of one execution, drawn by xterm.js: colours (ANSI), scrolling, selection and
 * search come from it. It does not own the process: it shows what the `TerminalHub` has for the
 * execution and reports keystrokes and size changes upward. xterm is loaded on first use.
 */
export function TerminalView({
  executionId,
  hub,
  label,
  readOnly,
  autoScroll,
  onInterrupt,
  onInput,
  onResize,
  ref,
}: Props) {
  const host = useRef<HTMLDivElement>(null);
  const terminal = useRef<Terminal | null>(null);
  const search = useRef<SearchAddon | null>(null);
  // Read by the long-lived xterm callbacks, so they always see the latest props.
  const latest = useRef({ readOnly, autoScroll, onInterrupt, onInput, onResize });
  useEffect(() => {
    latest.current = { readOnly, autoScroll, onInterrupt, onInput, onResize };
  });

  useImperativeHandle(
    ref,
    () => ({
      copy: async () => {
        const term = terminal.current;
        if (!term) return;
        const hadSelection = term.hasSelection();
        if (!hadSelection) term.selectAll();
        const text = term.getSelection();
        if (!hadSelection) term.clearSelection();
        try {
          await navigator.clipboard.writeText(text);
        } catch {
          // Copy is best-effort: the clipboard can be unavailable or refused.
        }
      },
      clear: () => {
        terminal.current?.clear();
        hub.clear(executionId);
      },
      find: (query, direction) => {
        if (query === '') return false;
        const addon = search.current;
        if (!addon) return false;
        return direction === 'next' ? addon.findNext(query) : addon.findPrevious(query);
      },
    }),
    [hub, executionId],
  );

  useEffect(() => {
    let disposed = false;
    let teardown: (() => void) | undefined;
    void Promise.all([
      import('@xterm/xterm'),
      import('@xterm/addon-fit'),
      import('@xterm/addon-search'),
    ]).then(([xterm, fitModule, searchModule]) => {
      const element = host.current;
      if (disposed || !element) return;
      const term = new xterm.Terminal({
        disableStdin: latest.current.readOnly,
        scrollback: SCROLLBACK_LINES,
        fontFamily: cssColor(element, '--font-mono', 'monospace'),
        fontSize: 12,
        cursorBlink: false,
        theme: {
          background: cssColor(element, '--color-bg', '#14161a'),
          foreground: cssColor(element, '--color-text', '#e8eaed'),
          cursor: cssColor(element, '--color-accent', '#748ffc'),
        },
      });
      const fit = new fitModule.FitAddon();
      const finder = new searchModule.SearchAddon();
      term.loadAddon(fit);
      term.loadAddon(finder);
      term.open(element);
      terminal.current = term;
      search.current = finder;

      term.attachCustomKeyEventHandler((event) => {
        if (event.type !== 'keydown') return true;
        if (event.ctrlKey && !event.metaKey && !event.altKey && event.key.toLowerCase() === 'c') {
          if (term.hasSelection()) {
            // Ctrl+C with a selection is a copy, like in any terminal.
            void navigator.clipboard.writeText(term.getSelection()).catch(() => undefined);
          } else {
            latest.current.onInterrupt();
          }
          return false;
        }
        // A read-only terminal must never trap the keyboard.
        return !(event.key === 'Tab' && latest.current.readOnly);
      });
      const input = term.onData((data) => {
        const { readOnly: locked, onInput: send } = latest.current;
        if (!locked) send?.(data);
      });

      const write = (data: string) => {
        const before = term.buffer.active.viewportY;
        term.write(data, () => {
          if (latest.current.autoScroll) term.scrollToBottom();
          else term.scrollToLine(before);
        });
      };
      write(hub.read(executionId));
      const unsubscribe = hub.subscribe(executionId, write);

      let timer: ReturnType<typeof setTimeout> | undefined;
      const sizeChanged = term.onResize(({ cols, rows }) => {
        clearTimeout(timer);
        timer = setTimeout(() => {
          if (cols >= MIN_COLS && rows >= MIN_ROWS) latest.current.onResize?.(cols, rows);
        }, RESIZE_DEBOUNCE_MS);
      });
      const refit = () => {
        try {
          fit.fit();
        } catch {
          // The element can be hidden or zero-sized for a moment.
        }
      };
      refit();
      const observer = new ResizeObserver(refit);
      observer.observe(element);

      teardown = () => {
        clearTimeout(timer);
        observer.disconnect();
        unsubscribe();
        sizeChanged.dispose();
        input.dispose();
        terminal.current = null;
        search.current = null;
        term.dispose();
      };
    });
    return () => {
      disposed = true;
      teardown?.();
    };
  }, [executionId, hub]);

  useEffect(() => {
    if (terminal.current) terminal.current.options.disableStdin = readOnly;
  }, [readOnly]);

  return <div ref={host} className={styles.host} role="region" aria-label={label} />;
}
