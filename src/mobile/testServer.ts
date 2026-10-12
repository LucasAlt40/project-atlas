import { vi } from 'vitest';

/**
 * A stand-in for the Atlas server and for the browser's EventSource, to test the page the way a
 * phone uses it: through `fetch` and a stream, never by reaching into its state.
 */

export interface Call {
  method: string;
  path: string;
  body: unknown;
}

type Handler = (call: Call) => Response | Promise<Response> | undefined;

export function json(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  });
}

export function problem(status: number, code: string, extra: object = {}): Response {
  return json(status, { error: { code, ...extra } });
}

export class FakeEventSource {
  static instances: FakeEventSource[] = [];
  static readonly CLOSED = 2;
  readyState = 0;
  private readonly listeners = new Map<string, ((event: MessageEvent<string>) => void)[]>();

  constructor(readonly url: string) {
    FakeEventSource.instances.push(this);
  }

  addEventListener(type: string, listener: (event: MessageEvent<string>) => void) {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]);
  }

  close() {
    this.readyState = FakeEventSource.CLOSED;
  }

  /** What the server sends. */
  emit(type: string, data: unknown = {}) {
    if (type === 'open') this.readyState = 1;
    for (const listener of this.listeners.get(type) ?? []) {
      listener(new MessageEvent(type, { data: JSON.stringify(data) }));
    }
  }

  static get current(): FakeEventSource {
    const last = FakeEventSource.instances.at(-1);
    if (!last) throw new Error('no stream was opened');
    return last;
  }
}

export interface FakeServer {
  calls: Call[];
  /** While true every request fails the way it does when the computer cannot be reached. */
  offline: boolean;
  /** Answers `method path` with what `handler` returns, newest first. */
  on: (method: string, path: string | RegExp, handler: (call: Call) => Response) => void;
  count: (method: string, path: string | RegExp) => number;
}

export function installServer(): FakeServer {
  FakeEventSource.instances = [];
  vi.stubGlobal('EventSource', FakeEventSource);
  const calls: Call[] = [];
  // eslint-disable-next-line prefer-const -- the stub below reads it, and it is built after
  let server: FakeServer;
  const handlers: { method: string; path: string | RegExp; handler: Handler }[] = [];
  const matches = (pattern: string | RegExp, path: string) =>
    typeof pattern === 'string' ? pattern === path : pattern.test(path);

  vi.stubGlobal(
    'fetch',
    vi.fn((input: string, init?: RequestInit) => {
      const call: Call = {
        method: init?.method ?? 'GET',
        path: input,
        body: typeof init?.body === 'string' ? (JSON.parse(init.body) as unknown) : undefined,
      };
      calls.push(call);
      if (server.offline) return Promise.reject(new TypeError('network'));
      for (const entry of handlers) {
        if (entry.method === call.method && matches(entry.path, call.path)) {
          const response = entry.handler(call);
          if (response) return Promise.resolve(response);
        }
      }
      return Promise.resolve(problem(404, 'not_found'));
    }),
  );

  server = {
    calls,
    offline: false,
    on: (method, path, handler) => {
      handlers.unshift({ method, path, handler });
    },
    count: (method, path) =>
      calls.filter((call) => call.method === method && matches(path, call.path)).length,
  };
  return server;
}
