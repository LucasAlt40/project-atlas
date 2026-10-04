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

// The graph library reads layout that jsdom does not have: sizes, the transform matrix, and
// the bounding box of SVG elements. Nodes are given an initial size, so none of it matters
// to what the tests check.
class NoopDOMMatrixReadOnly {
  m22 = 1;
  constructor(transform?: string) {
    const scale = /scale\(([\d.]+)\)/.exec(transform ?? '');
    if (scale?.[1]) this.m22 = Number(scale[1]);
  }
}
if (!('DOMMatrixReadOnly' in globalThis)) vi.stubGlobal('DOMMatrixReadOnly', NoopDOMMatrixReadOnly);
