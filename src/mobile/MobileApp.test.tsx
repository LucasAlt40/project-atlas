import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { detail, overview, question, summary } from './fixtures';
import { MobileApp } from './MobileApp';
import { FakeEventSource, installServer, json, problem, type FakeServer } from './testServer';

const SESSION = {
  device: { id: 'dev-1', name: 'Pixel' },
  serverTime: Date.now(),
  epoch: 'e1',
  language: 'en-US',
};

let server: FakeServer;

/** A paired phone with these runs, and `details` by run id. */
function paired(runs = [summary()], details: Record<string, ReturnType<typeof detail>> = {}) {
  server.on('GET', '/api/session', () => json(200, SESSION));
  server.on('GET', '/api/runs', () => json(200, overview(runs)));
  server.on('GET', /^\/api\/runs\/[^/]+$/, (call) => {
    const run = details[decodeURIComponent(call.path.split('/').at(-1) ?? '')];
    return run ? json(200, run) : problem(404, 'not_found');
  });
}

function openRun(id = 'run-1') {
  window.location.hash = `#/run/${id}`;
  render(<MobileApp />);
}

describe('the phone page', () => {
  beforeEach(() => {
    server = installServer();
    window.location.hash = '';
    document.title = '';
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  describe('pairing', () => {
    it('asks for a code when there is no session', async () => {
      server.on('GET', '/api/session', () => problem(401, 'unauthenticated'));
      render(<MobileApp />);

      expect(await screen.findByLabelText('Pairing code')).toBeInTheDocument();
      expect(screen.queryByText('Needs you')).not.toBeInTheDocument();
    });

    it('pairs by itself with the code in the link, once, and does not leave it in the address', async () => {
      window.location.hash = '#ABCD-EFGH-IJKL-MNOP';
      server.on('GET', '/api/session', () => problem(401, 'unauthenticated'));
      server.on('POST', '/api/pair', () => json(200, SESSION));
      server.on('GET', '/api/runs', () => json(200, overview([summary()])));

      render(<MobileApp />);

      expect(await screen.findByText('Password recovery')).toBeInTheDocument();
      expect(server.count('POST', '/api/pair')).toBe(1);
      expect(server.calls.find((c) => c.path === '/api/pair')?.body).toMatchObject({
        code: 'ABCDEFGHIJKLMNOP',
        deviceName: expect.stringMatching(/.+/) as string,
      });
      expect(window.location.hash).not.toContain('ABCD');
      expect(window.location.href).not.toContain('ABCD');
    });

    it('sends the code of the link only once, not again after a disconnect', async () => {
      const user = userEvent.setup();
      window.location.hash = '#ABCD-EFGH-IJKL-MNOP';
      server.on('GET', '/api/session', () => problem(401, 'unauthenticated'));
      server.on('POST', '/api/pair', () => json(200, SESSION));
      server.on('GET', '/api/runs', () => json(200, overview([summary()])));
      server.on('DELETE', '/api/session', () => json(200, { ok: true }));
      render(<MobileApp />);

      await user.click(await screen.findByRole('button', { name: 'Disconnect this phone' }));

      expect(await screen.findByLabelText('Pairing code')).toBeInTheDocument();
      // A second try of the spent code would count on the computer as a wrong one.
      expect(server.count('POST', '/api/pair')).toBe(1);
      expect(screen.queryByText(/not valid/)).not.toBeInTheDocument();
    });

    it('takes a code typed in, however it is written', async () => {
      const user = userEvent.setup();
      server.on('GET', '/api/session', () => problem(401, 'unauthenticated'));
      server.on('POST', '/api/pair', () => json(200, SESSION));
      server.on('GET', '/api/runs', () => json(200, overview([])));
      render(<MobileApp />);

      await user.type(await screen.findByLabelText('Pairing code'), 'abcd efgh-ijkl mnop');
      await user.click(screen.getByRole('button', { name: 'Connect' }));

      expect(await screen.findByText(/No runs yet/)).toBeInTheDocument();
      expect(server.calls.find((c) => c.path === '/api/pair')?.body).toMatchObject({
        code: 'ABCDEFGHIJKLMNOP',
      });
    });

    it('does not even ask the server about a code that cannot be one', async () => {
      const user = userEvent.setup();
      server.on('GET', '/api/session', () => problem(401, 'unauthenticated'));
      render(<MobileApp />);

      await user.type(await screen.findByLabelText('Pairing code'), 'nope');
      await user.click(screen.getByRole('button', { name: 'Connect' }));

      expect(await screen.findByRole('alert')).toHaveTextContent('not valid');
      expect(server.count('POST', '/api/pair')).toBe(0);
    });

    it('says when pairing is closed after too many wrong codes, and for how long', async () => {
      const user = userEvent.setup();
      server.on('GET', '/api/session', () => problem(401, 'unauthenticated'));
      server.on('POST', '/api/pair', () =>
        problem(429, 'pairing_locked', { retryAfterMs: 60_000 }),
      );
      render(<MobileApp />);

      await user.type(await screen.findByLabelText('Pairing code'), 'ABCDEFGHIJKLMNOP');
      await user.click(screen.getByRole('button', { name: 'Connect' }));

      expect(await screen.findByRole('alert')).toHaveTextContent('closed for 60 seconds');
    });

    it('explains a rejected code and says what to do', async () => {
      const user = userEvent.setup();
      server.on('GET', '/api/session', () => problem(401, 'unauthenticated'));
      server.on('POST', '/api/pair', () => problem(403, 'invalid_code'));
      render(<MobileApp />);

      await user.type(await screen.findByLabelText('Pairing code'), 'ABCDEFGHIJKLMNOP');
      await user.click(screen.getByRole('button', { name: 'Connect' }));

      expect(await screen.findByRole('alert')).toHaveTextContent('ask for a new one');
    });
  });

  describe('the dashboard', () => {
    it('puts what waits for a person first, and says what the engine says', async () => {
      paired([
        summary({
          id: 'done',
          workflowName: 'Finished flow',
          status: 'completed',
          completedAt: 1_090_000,
          current: [],
        }),
        summary({ id: 'busy', workflowName: 'Busy flow' }),
        summary({
          id: 'ask',
          workflowName: 'Asking flow',
          status: 'waiting_for_input',
          attention: 'needs_answer',
          pending: 1,
        }),
        summary({
          id: 'bad',
          workflowName: 'Broken flow',
          status: 'failed',
          attention: 'failed',
          failure: { code: 'node_failed', step: 'QA', detail: null },
        }),
      ]);
      render(<MobileApp />);

      await screen.findByText('Asking flow');
      const headings = screen.getAllByRole('heading', { level: 2 }).map((h) => h.textContent);
      expect(headings.map((h) => h.replace(/\d+$/, '').trim())).toEqual([
        'Needs you',
        'Running',
        'Failed',
        'Recently finished',
      ]);
      const asking = screen.getByRole('link', { name: /Asking flow/ });
      expect(asking).toHaveTextContent('A question is waiting for your answer');
      expect(screen.getByRole('link', { name: /Broken flow/ })).toHaveTextContent(
        'The step “QA” failed.',
      );
      // Steps are counted; nothing pretends to know a percentage.
      expect(screen.getAllByText('1 of 3 steps done').length).toBeGreaterThan(0);
      expect(document.body.textContent).not.toMatch(/\d+\s?%/);
    });

    it('tells the person when nothing is running yet', async () => {
      paired([]);
      render(<MobileApp />);
      expect(await screen.findByText(/Start a workflow on the computer/)).toBeInTheDocument();
    });

    it('puts the number waiting in the tab title', async () => {
      paired([summary({ attention: 'needs_answer', pending: 1, status: 'waiting_for_input' })]);
      render(<MobileApp />);
      await screen.findByText('Password recovery');
      await waitFor(() => {
        expect(document.title).toBe('(1) Atlas');
      });
    });

    it('speaks Portuguese when the computer is set to it', async () => {
      server.on('GET', '/api/session', () => json(200, { ...SESSION, language: 'pt-BR' }));
      server.on('GET', '/api/runs', () =>
        json(
          200,
          overview(
            [summary({ attention: 'needs_answer', pending: 2, status: 'waiting_for_input' })],
            { language: 'pt-BR' },
          ),
        ),
      );
      render(<MobileApp />);
      expect(await screen.findByText('2 perguntas esperam sua resposta')).toBeInTheDocument();
      expect(screen.getByText('Precisa de você')).toBeInTheDocument();
    });
  });

  describe('a run', () => {
    it('shows the plan, what it is about, who asked, and the steps as they really are', async () => {
      paired([summary()], { 'run-1': detail() });
      openRun();

      expect(
        await screen.findByRole('heading', { name: 'Do you approve this plan?' }),
      ).toBeInTheDocument();
      expect(screen.getByText('The plan')).toBeInTheDocument();
      expect(screen.getByText('add the route')).toBeInTheDocument();
      expect(screen.getByText('Plan to approve')).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Approve' })).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Request changes' })).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Redo the plan' })).toBeInTheDocument();
      // The timeline: what ran, what waits for the person, what has not begun.
      const steps = within(screen.getByRole('list', { name: 'Steps' })).getAllByRole('listitem');
      expect(steps.map((s) => s.textContent)).toEqual([
        expect.stringContaining('Architect'),
        expect.stringContaining('Developer'),
        expect.stringContaining('QA'),
      ]);
      expect(steps[0]).toHaveTextContent('Done');
      expect(steps[1]).toHaveTextContent('Waiting for you');
      expect(steps[2]).toHaveTextContent('Not started');
      expect(steps[1]).toHaveTextContent('Then: QA');
    });

    it('approves through the server and shows what the server confirmed', async () => {
      const user = userEvent.setup();
      const answered = detail({
        status: 'running',
        attention: 'none',
        pending: 0,
        interactions: [question({ status: 'answered', choice: 'approve', answeredAt: 1_070_000 })],
      });
      paired([summary()], { 'run-1': detail() });
      server.on('POST', '/api/runs/run-1/interactions/q-1/answer', () =>
        json(200, { applied: 'now', run: answered }),
      );
      openRun();

      await user.click(await screen.findByRole('button', { name: 'Approve' }));

      expect(await screen.findByText('Sent. The computer confirmed it.')).toBeInTheDocument();
      expect(server.calls.find((c) => c.method === 'POST')?.body).toEqual({ choice: 'approve' });
      // The question is now history, not a button.
      expect(screen.queryByRole('button', { name: 'Approve' })).not.toBeInTheDocument();
      expect(screen.getByText(/Developer · Approve/)).toBeInTheDocument();
    });

    it('does not let a plan go back for changes without saying what to change', async () => {
      const user = userEvent.setup();
      paired([summary()], { 'run-1': detail() });
      server.on('POST', /answer$/, () => json(200, { applied: 'now', run: detail() }));
      openRun();

      await user.click(await screen.findByRole('button', { name: 'Request changes' }));
      expect(server.count('POST', /answer$/)).toBe(0);
      expect(screen.getByText(/Say what to change/)).toBeInTheDocument();

      await user.type(screen.getByLabelText('What should change?'), 'Split it in two');
      await user.click(screen.getByRole('button', { name: 'Request changes' }));

      expect(server.calls.find((c) => c.method === 'POST')?.body).toEqual({
        choice: 'revise',
        text: 'Split it in two',
      });
    });

    it('answers a question in words', async () => {
      const user = userEvent.setup();
      const ask = question({
        id: 'q-2',
        kind: 'clarification',
        question: 'Which API should I use?',
        document: '',
        options: [],
      });
      paired([summary()], { 'run-1': detail({ interactions: [ask] }) });
      server.on('POST', /answer$/, () =>
        json(200, { applied: 'now', run: detail({ interactions: [] }) }),
      );
      openRun();

      const send = await screen.findByRole('button', { name: 'Send answer' });
      expect(send).toBeDisabled();
      await user.type(screen.getByLabelText('Your answer'), 'Use v2');
      await user.click(send);

      await waitFor(() => {
        expect(server.calls.find((c) => c.method === 'POST')?.body).toEqual({ text: 'Use v2' });
      });
    });

    it('tells the person when the computer got there first, and shows what was decided', async () => {
      const user = userEvent.setup();
      paired([summary()], { 'run-1': detail() });
      server.on('POST', /answer$/, () => problem(409, 'already_resolved'));
      openRun();
      await user.click(await screen.findByRole('button', { name: 'Approve' }));

      expect(await screen.findByRole('alert')).toHaveTextContent('Someone already answered this');
      // And it reads the run again, so the page does not go on offering a closed question.
      await waitFor(() => {
        expect(server.count('GET', '/api/runs/run-1')).toBeGreaterThan(1);
      });
    });

    it('says nothing was sent when the computer cannot be reached, and keeps the question', async () => {
      const user = userEvent.setup();
      paired([summary()], { 'run-1': detail() });
      openRun();
      const approve = await screen.findByRole('button', { name: 'Approve' });
      server.offline = true;

      await user.click(approve);

      expect(await screen.findByRole('alert')).toHaveTextContent('Nothing was sent');
      expect(screen.getByRole('button', { name: 'Approve' })).toBeEnabled();
    });

    it('only lets a permission be refused, and says why', async () => {
      paired([summary()], {
        'run-1': detail({
          interactions: [
            question({
              id: 'q-3',
              kind: 'permission',
              question: 'May I run the migrations?',
              document: '',
              options: [
                { id: 'allow', label: 'Allow' },
                { id: 'deny', label: 'Deny' },
              ],
              remote: { canAnswer: true, choices: ['deny'], desktopOnly: 'grants_authority' },
            }),
          ],
        }),
      });
      openRun();

      expect(await screen.findByRole('button', { name: 'Deny' })).toBeInTheDocument();
      expect(screen.queryByRole('button', { name: 'Allow' })).not.toBeInTheDocument();
      expect(screen.getByText(/you can only refuse/)).toBeInTheDocument();
    });

    it('offers no way to answer what is decided at the computer', async () => {
      paired([summary()], {
        'run-1': detail({
          interactions: [
            question({
              id: 'q-4',
              kind: 'approval',
              question: 'Send this context to the model?',
              document: '',
              options: [
                { id: 'approve', label: 'Approve' },
                { id: 'reject', label: 'Reject' },
              ],
              remote: { canAnswer: false, choices: [], desktopOnly: 'guardrail' },
            }),
          ],
        }),
      });
      openRun();

      expect(await screen.findByText(/decided on the computer/)).toBeInTheDocument();
      expect(screen.queryByRole('button', { name: 'Approve' })).not.toBeInTheDocument();
      expect(screen.queryByRole('button', { name: 'Reject' })).not.toBeInTheDocument();
    });

    it('shows why a run failed and that recovering is for the computer', async () => {
      paired([summary()], {
        'run-1': detail({
          status: 'failed',
          attention: 'failed',
          pending: 0,
          interactions: [],
          failure: { code: 'node_failed', step: 'QA', detail: 'tests failed' },
          controls: { pause: false, resume: false, cancel: false },
          completedAt: 1_080_000,
        }),
      });
      openRun();

      expect(await screen.findByText(/The step “QA” failed\. — tests failed/)).toBeInTheDocument();
      expect(screen.getByText(/Recovering is done on the computer/)).toBeInTheDocument();
      expect(screen.queryByRole('button', { name: 'Cancel the run' })).not.toBeInTheDocument();
    });

    it('shows the result of a run that finished, and that the code is applied at the computer', async () => {
      paired([summary()], {
        'run-1': detail({
          status: 'completed',
          attention: 'none',
          pending: 0,
          interactions: [],
          result: {
            summary: 'Everything passes.',
            artifacts: [{ name: 'api.md', kind: 'api_contract', summary: 'The contract' }],
            decisions: [{ title: 'Use JWT', decision: 'for the reset token' }],
          },
          code: { status: 'changes_available', filesChanged: 4, additions: 30, deletions: 2 },
          controls: { pause: false, resume: false, cancel: false },
          completedAt: 1_090_000,
        }),
      });
      openRun();

      expect(await screen.findByText('Everything passes.')).toBeInTheDocument();
      expect(screen.getByText('api.md')).toBeInTheDocument();
      expect(screen.getByText('4 files changed (+30 −2)')).toBeInTheDocument();
      expect(screen.getByText(/applying the code is done on the computer/i)).toBeInTheDocument();
    });

    it('does not show a run that is gone', async () => {
      paired([summary()], {});
      openRun('ghost');
      expect(await screen.findByText('This run no longer exists.')).toBeInTheDocument();
    });
  });

  describe('controlling a run', () => {
    it('asks again before cancelling, and says it was meant', async () => {
      const user = userEvent.setup();
      paired([summary()], { 'run-1': detail() });
      server.on('POST', '/api/runs/run-1/control', () =>
        json(
          200,
          detail({
            status: 'cancelled',
            attention: 'none',
            controls: { pause: false, resume: false, cancel: false },
          }),
        ),
      );
      openRun();

      await user.click(await screen.findByRole('button', { name: 'Cancel the run' }));
      expect(server.count('POST', '/api/runs/run-1/control')).toBe(0);
      const sure = screen.getByRole('alertdialog');
      expect(sure).toHaveTextContent('This cannot be undone');
      await user.click(within(sure).getByRole('button', { name: 'Keep it running' }));
      expect(server.count('POST', '/api/runs/run-1/control')).toBe(0);

      await user.click(screen.getByRole('button', { name: 'Cancel the run' }));
      await user.click(
        within(screen.getByRole('alertdialog')).getByRole('button', {
          name: 'Yes, cancel the run',
        }),
      );

      await waitFor(() => {
        expect(server.calls.find((c) => c.path === '/api/runs/run-1/control')?.body).toEqual({
          action: 'cancel',
          confirm: true,
        });
      });
    });

    it('pauses without asking, and resumes', async () => {
      const user = userEvent.setup();
      paired([summary()], {
        'run-1': detail({
          interactions: [],
          controls: { pause: true, resume: false, cancel: true },
        }),
      });
      server.on('POST', '/api/runs/run-1/control', () =>
        json(
          200,
          detail({
            status: 'paused',
            interactions: [],
            controls: { pause: false, resume: true, cancel: true },
          }),
        ),
      );
      openRun();

      await user.click(await screen.findByRole('button', { name: 'Pause' }));

      expect(await screen.findByRole('button', { name: 'Resume' })).toBeInTheDocument();
      expect(server.calls.find((c) => c.method === 'POST')?.body).toEqual({
        action: 'pause',
        confirm: false,
      });
    });
  });

  describe('staying in step with the computer', () => {
    it('reads the runs again when the stream says something changed', async () => {
      paired([summary()]);
      render(<MobileApp />);
      await screen.findByText('Password recovery');
      const before = server.count('GET', '/api/runs');

      act(() => {
        FakeEventSource.current.emit('open');
        FakeEventSource.current.emit('change', {
          seq: 1,
          kind: 'node_started',
          runId: 'run-1',
          workspaceId: 'ws-1',
          at: 1,
        });
      });

      await waitFor(() => {
        expect(server.count('GET', '/api/runs')).toBeGreaterThan(before);
      });
    });

    it('tells the person when a question starts waiting, and where to go', async () => {
      const user = userEvent.setup();
      let runs = [summary()];
      server.on('GET', '/api/session', () => json(200, SESSION));
      server.on('GET', '/api/runs', () => json(200, overview(runs)));
      server.on('GET', /^\/api\/runs\/run-1$/, () => json(200, detail()));
      render(<MobileApp />);
      await screen.findByText('Password recovery');

      runs = [summary({ attention: 'needs_answer', pending: 1, status: 'waiting_for_input' })];
      act(() => {
        FakeEventSource.current.emit('change', {
          seq: 2,
          kind: 'interaction_detected',
          runId: 'run-1',
          workspaceId: 'ws-1',
          at: 2,
        });
      });

      const alert = await screen.findByRole('alert');
      expect(alert).toHaveTextContent('Waiting for your answer');
      expect(alert).toHaveTextContent('Password recovery');
      await user.click(within(alert).getByRole('button', { name: /Waiting for your answer/ }));
      expect(
        await screen.findByRole('heading', { name: 'Do you approve this plan?' }),
      ).toBeInTheDocument();
      expect(screen.queryByText('Waiting for your answer')).not.toBeInTheDocument();
    });

    it('tells the person when a run fails or finishes', async () => {
      let runs = [
        summary({ id: 'a', workflowName: 'Flow A' }),
        summary({ id: 'b', workflowName: 'Flow B' }),
      ];
      server.on('GET', '/api/session', () => json(200, SESSION));
      server.on('GET', '/api/runs', () => json(200, overview(runs)));
      render(<MobileApp />);
      await screen.findByText('Flow A');

      runs = [
        summary({
          id: 'a',
          workflowName: 'Flow A',
          status: 'failed',
          attention: 'failed',
          failure: { code: 'node_failed', step: 'QA', detail: null },
        }),
        summary({
          id: 'b',
          workflowName: 'Flow B',
          status: 'completed',
          completedAt: 5,
          current: [],
        }),
      ];
      act(() => {
        FakeEventSource.current.emit('resync', { epoch: 'e1', seq: 5 });
      });

      expect(await screen.findByText('A run failed')).toBeInTheDocument();
      expect(screen.getByText('A run finished')).toBeInTheDocument();
    });

    it('says the computer cannot be reached instead of showing old data as live, and recovers', async () => {
      paired([summary()]);
      render(<MobileApp />);
      await screen.findByText('Password recovery');

      server.offline = true;
      act(() => {
        FakeEventSource.current.emit('error');
        FakeEventSource.current.emit('change', {
          seq: 1,
          kind: 'node_started',
          runId: 'run-1',
          workspaceId: 'ws-1',
          at: 1,
        });
      });

      const banner = await screen.findByText('Can’t reach the computer.');
      expect(banner.closest('[role=status]')).toHaveTextContent('Last update');
      // What was known stays on screen.
      expect(screen.getByText('Password recovery')).toBeInTheDocument();

      server.offline = false;
      act(() => {
        FakeEventSource.current.emit('change', {
          seq: 2,
          kind: 'node_started',
          runId: 'run-1',
          workspaceId: 'ws-1',
          at: 2,
        });
        FakeEventSource.current.emit('open');
      });
      await waitFor(() => {
        expect(screen.queryByText('Can’t reach the computer.')).not.toBeInTheDocument();
      });
    });

    it('calls the computer unreachable when the stream drops and nothing answers', async () => {
      paired([summary()]);
      render(<MobileApp />);
      await screen.findByText('Password recovery');

      server.offline = true;
      act(() => {
        FakeEventSource.current.emit('error');
      });

      expect(await screen.findByText('Reconnecting…')).toBeInTheDocument();
      // Nothing answers: after a moment the page says what that probably means.
      expect(
        await screen.findByText('Can’t reach the computer.', {}, { timeout: 4000 }),
      ).toBeInTheDocument();
      expect(screen.getByText(/off, asleep or off the network/)).toBeInTheDocument();
    });

    it('goes back to pairing, and says why, when the computer revokes the phone', async () => {
      paired([summary()]);
      render(<MobileApp />);
      await screen.findByText('Password recovery');

      act(() => {
        FakeEventSource.current.emit('revoked');
      });

      expect(await screen.findByLabelText('Pairing code')).toBeInTheDocument();
      expect(screen.getByText(/access ended/)).toBeInTheDocument();
      expect(screen.queryByText('Password recovery')).not.toBeInTheDocument();
    });

    it('goes back to pairing when a request finds the session gone', async () => {
      let alive = true;
      server.on('GET', '/api/session', () => json(200, SESSION));
      server.on('GET', '/api/runs', () =>
        alive ? json(200, overview([summary()])) : problem(401, 'unauthenticated'),
      );
      render(<MobileApp />);
      await screen.findByText('Password recovery');

      alive = false;
      act(() => {
        FakeEventSource.current.emit('change', {
          seq: 1,
          kind: 'x',
          runId: null,
          workspaceId: null,
          at: 1,
        });
      });

      expect(await screen.findByLabelText('Pairing code')).toBeInTheDocument();
    });

    it('says the session ran out when the computer says it expired', async () => {
      server.on('GET', '/api/session', () => problem(401, 'session_expired'));
      render(<MobileApp />);
      expect(await screen.findByText(/session expired/)).toBeInTheDocument();
    });

    it('disconnects this phone on request', async () => {
      const user = userEvent.setup();
      paired([summary()]);
      server.on('DELETE', '/api/session', () => json(200, { ok: true }));
      render(<MobileApp />);

      await user.click(await screen.findByRole('button', { name: 'Disconnect this phone' }));

      expect(server.count('DELETE', '/api/session')).toBe(1);
      expect(await screen.findByLabelText('Pairing code')).toBeInTheDocument();
    });

    it('stays paired, and says so, when the computer does not confirm the disconnect', async () => {
      const user = userEvent.setup();
      paired([summary()]);
      server.on('DELETE', '/api/session', () => problem(403, 'bad_origin'));
      render(<MobileApp />);

      await user.click(await screen.findByRole('button', { name: 'Disconnect this phone' }));

      expect(await screen.findByRole('alert')).toHaveTextContent(/could not be disconnected/);
      expect(screen.queryByLabelText('Pairing code')).not.toBeInTheDocument();
      expect(screen.getByText('Password recovery')).toBeInTheDocument();
    });

    it('reads the run again when the page comes back to the foreground', async () => {
      paired([summary()]);
      render(<MobileApp />);
      await screen.findByText('Password recovery');
      const before = server.count('GET', '/api/runs');

      act(() => {
        document.dispatchEvent(new Event('visibilitychange'));
      });

      await waitFor(() => {
        expect(server.count('GET', '/api/runs')).toBeGreaterThan(before);
      });
    });
  });
});
