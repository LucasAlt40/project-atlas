import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { afterEach, vi } from 'vitest';

afterEach(() => {
  cleanup();
});

// jsdom has no ResizeObserver; the terminal view watches its container with one.
class NoopResizeObserver {
  observe(): void {
    // Layout does not exist in jsdom.
  }
  unobserve(): void {
    // See above.
  }
  disconnect(): void {
    // See above.
  }
}
if (!('ResizeObserver' in globalThis)) vi.stubGlobal('ResizeObserver', NoopResizeObserver);
