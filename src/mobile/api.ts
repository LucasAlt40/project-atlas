import type { Answered, Overview, RunDetail, SessionInfo } from './types';

/** A failure the server named (`error.code`), or the network itself failing (`status` 0). */
export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    readonly retryAfterMs?: number,
  ) {
    super(`${String(status)} ${code}`);
    this.name = 'ApiError';
  }

  /** The server could not be reached at all (the computer is off or asleep, or the Wi-Fi is gone). */
  get unreachable(): boolean {
    return this.status === 0;
  }

  /** The session is gone: never paired, revoked, expired, or from another start of the server. */
  get unauthenticated(): boolean {
    return this.status === 401;
  }
}

interface ErrorBody {
  error?: { code?: string; retryAfterMs?: number };
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  let response: Response;
  try {
    const init: RequestInit = { method, credentials: 'same-origin', cache: 'no-store' };
    if (body !== undefined) {
      init.headers = { 'Content-Type': 'application/json' };
      init.body = JSON.stringify(body);
    }
    response = await fetch(path, init);
  } catch {
    throw new ApiError(0, 'unreachable');
  }
  if (!response.ok) {
    const parsed = (await response.json().catch(() => ({}))) as ErrorBody;
    throw new ApiError(
      response.status,
      parsed.error?.code ?? 'unknown',
      parsed.error?.retryAfterMs,
    );
  }
  return (await response.json()) as T;
}

export const api = {
  pair: (code: string, deviceName: string) =>
    request<SessionInfo>('POST', '/api/pair', { code, deviceName }),
  session: () => request<SessionInfo>('GET', '/api/session'),
  logout: () => request<{ ok: boolean }>('DELETE', '/api/session'),
  runs: () => request<Overview>('GET', '/api/runs'),
  run: (id: string) => request<RunDetail>('GET', `/api/runs/${encodeURIComponent(id)}`),
  answer: (runId: string, interactionId: string, answer: { choice?: string; text?: string }) =>
    request<Answered>(
      'POST',
      `/api/runs/${encodeURIComponent(runId)}/interactions/${encodeURIComponent(interactionId)}/answer`,
      answer,
    ),
  control: (runId: string, action: 'pause' | 'resume' | 'cancel', confirm = false) =>
    request<RunDetail>('POST', `/api/runs/${encodeURIComponent(runId)}/control`, {
      action,
      confirm,
    }),
};
