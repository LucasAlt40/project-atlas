// Design-review capture harness for the phone page: stubs the server (fetch + EventSource).
// ?phase=ok|unpaired|offline   ?runs=0..4   open a run with #/run/<id>
import { detail, overview, question, summary } from '@/mobile/fixtures';

const params = new URLSearchParams(location.search);
const phase = params.get('phase') ?? 'ok';
const runCount = Number(params.get('runs') ?? 4);
document.documentElement.lang = 'pt-BR';

const NOW = Date.now();
const MIN = 60_000;
const all = [
  summary({
    id: 'run-1',
    workflowName: 'Recuperação de senha',
    workspaceName: 'Transport ERP',
    task: 'Implementar a recuperação de senha por e-mail',
    status: 'waiting_for_input',
    attention: 'needs_answer',
    pending: 1,
    stepsDone: 1,
    stepsTotal: 3,
    startedAt: NOW - 14 * MIN,
    updatedAt: NOW - MIN,
  }),
  summary({
    id: 'run-2',
    workflowName: 'Revisão do faturamento',
    workspaceName: 'Transport ERP',
    task: 'Revisar a camada de faturamento',
    status: 'running',
    attention: 'none',
    stepsDone: 2,
    stepsTotal: 4,
    startedAt: NOW - 32 * MIN,
    updatedAt: NOW - 2 * MIN,
  }),
  summary({
    id: 'run-3',
    workflowName: 'Exportar CSV',
    workspaceName: 'Billing API',
    task: 'Exportar faturas em CSV',
    status: 'failed',
    attention: 'failed',
    stepsDone: 1,
    stepsTotal: 3,
    startedAt: NOW - 90 * MIN,
    updatedAt: NOW - 88 * MIN,
    completedAt: NOW - 88 * MIN,
    failure: { kind: 'runtime_error', message: 'O runtime encerrou com erro.' } as never,
  }),
  summary({
    id: 'run-4',
    workflowName: 'Modo escuro',
    workspaceName: 'Transport ERP',
    task: 'Adicionar o modo escuro',
    status: 'completed',
    attention: 'none',
    stepsDone: 3,
    stepsTotal: 3,
    startedAt: NOW - 5 * 60 * MIN,
    updatedAt: NOW - 4 * 60 * MIN,
    completedAt: NOW - 4 * 60 * MIN,
  }),
].slice(0, runCount);

const json = (status: number, body: unknown) =>
  new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } });

window.fetch = ((input: string, init?: RequestInit) => {
  const method = init?.method ?? 'GET';
  const path = String(input);
  if (phase === 'offline') return Promise.reject(new TypeError('network'));
  if (path === '/api/session' && method === 'GET') {
    if (phase === 'unpaired')
      return Promise.resolve(json(401, { error: { code: 'unauthenticated' } }));
    return Promise.resolve(
      json(200, {
        device: { id: 'd1', name: 'iPhone do Lucas' },
        serverTime: Date.now(),
        epoch: 'e1',
        language: 'pt-BR',
      }),
    );
  }
  if (path === '/api/pair')
    return Promise.resolve(
      json(200, {
        device: { id: 'd1', name: 'iPhone do Lucas' },
        serverTime: Date.now(),
        epoch: 'e1',
        language: 'pt-BR',
      }),
    );
  if (path === '/api/runs') return Promise.resolve(json(200, overview(all, { language: 'pt-BR' })));
  const m = /^\/api\/runs\/([^/]+)$/.exec(path);
  if (m && method === 'GET') {
    const base = all.find((r) => r.id === decodeURIComponent(m[1] ?? '')) ?? all[0];
    return Promise.resolve(
      json(
        200,
        detail({
          ...base,
          interactions:
            base?.attention === 'needs_answer'
              ? [
                  question({
                    question: 'Você aprova este plano?',
                    document:
                      '# O plano\n\n- criar a rota de recuperação\n- enviar o e-mail com o link\n- escrever os testes',
                    options: [
                      { id: 'approve', label: 'Aprovar o plano' },
                      { id: 'revise', label: 'Pedir alterações' },
                      { id: 'redo', label: 'Refazer o plano' },
                    ],
                  }),
                ]
              : [],
        }),
      ),
    );
  }
  return Promise.resolve(json(404, { error: { code: 'not_found' } }));
}) as typeof fetch;

class StubSource {
  static readonly CLOSED = 2;
  readyState = 1;
  private listeners = new Map<string, ((e: MessageEvent<string>) => void)[]>();
  constructor(readonly url: string) {
    setTimeout(() => this.emit('open'), 50);
  }
  addEventListener(t: string, l: (e: MessageEvent<string>) => void) {
    this.listeners.set(t, [...(this.listeners.get(t) ?? []), l]);
  }
  close() {
    this.readyState = 2;
  }
  emit(t: string) {
    for (const l of this.listeners.get(t) ?? []) l(new MessageEvent(t, { data: '{}' }));
  }
}
(window as unknown as { EventSource: unknown }).EventSource = StubSource;

void import('@/mobile/main');
