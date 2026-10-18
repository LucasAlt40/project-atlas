import { describe, expect, it, vi } from 'vitest';
import { subscribe } from './subscribe';

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('subscribe', () => {
  it('closes an open subscription', async () => {
    const unlisten = vi.fn();
    const close = subscribe(() => Promise.resolve(unlisten));
    await settle();

    close();

    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('closes a subscription that was left before it was ready, as soon as it is', async () => {
    const unlisten = vi.fn();
    let ready: (stop: () => void) => void = () => undefined;
    const close = subscribe(
      () =>
        new Promise<() => void>((resolve) => {
          ready = resolve;
        }),
    );

    close();
    expect(unlisten).not.toHaveBeenCalled();
    ready(unlisten);
    await settle();

    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('a subscription that cannot open is not an unhandled error', async () => {
    const close = subscribe(() => Promise.reject(new Error('no ipc')));
    await settle();

    expect(close).not.toThrow();
  });
});
