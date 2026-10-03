import { vi } from 'vitest';

/**
 * xterm.js needs a real layout engine; in tests the terminal view talks to this stand-in, which
 * records what the app asks of it. Mount it with
 * `vi.mock('@xterm/xterm', async () => (await import('@/test/xtermMock')).xtermModule)` (and the
 * addons likewise).
 */
export class FakeTerminal {
  static instances: FakeTerminal[] = [];

  written: string[] = [];
  options: { disableStdin?: boolean };
  selection = '';
  disposed = false;
  cleared = 0;
  keyHandler: ((event: KeyboardEvent) => boolean) | undefined;
  dataHandler: ((data: string) => void) | undefined;
  resizeHandler: ((size: { cols: number; rows: number }) => void) | undefined;
  buffer = { active: { viewportY: 0 } };

  constructor(options: { disableStdin?: boolean } = {}) {
    this.options = options;
    FakeTerminal.instances.push(this);
  }

  /** Everything written so far, as one text. */
  get text(): string {
    return this.written.join('');
  }

  loadAddon = vi.fn();
  open = vi.fn();
  focus = vi.fn();
  scrollToBottom = vi.fn();
  scrollToLine = vi.fn();
  selectAll = vi.fn(() => {
    this.selection = this.text;
  });
  clearSelection = vi.fn(() => {
    this.selection = '';
  });
  hasSelection = () => this.selection !== '';
  getSelection = () => this.selection;

  write(data: string, callback?: () => void): void {
    if (data !== '') this.written.push(data);
    callback?.();
  }

  clear(): void {
    this.cleared += 1;
    this.written = [];
  }

  attachCustomKeyEventHandler(handler: (event: KeyboardEvent) => boolean): void {
    this.keyHandler = handler;
  }

  onData(handler: (data: string) => void) {
    this.dataHandler = handler;
    return { dispose: vi.fn() };
  }

  onResize(handler: (size: { cols: number; rows: number }) => void) {
    this.resizeHandler = handler;
    return { dispose: vi.fn() };
  }

  dispose(): void {
    this.disposed = true;
  }
}

export class FakeFitAddon {
  fit = vi.fn();
}

export class FakeSearchAddon {
  static last: FakeSearchAddon | undefined;
  findNext = vi.fn(() => true);
  findPrevious = vi.fn(() => true);
  constructor() {
    FakeSearchAddon.last = this;
  }
}

export const xtermModule = { Terminal: FakeTerminal };
export const fitModule = { FitAddon: FakeFitAddon };
export const searchModule = { SearchAddon: FakeSearchAddon };

/** The terminal most recently opened by the app. */
export function lastTerminal(): FakeTerminal {
  const terminal = FakeTerminal.instances.at(-1);
  if (!terminal) throw new Error('no terminal was opened');
  return terminal;
}
