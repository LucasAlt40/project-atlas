import {
  conversationsReducer,
  initialConversations,
  type ConversationAction,
  type ConversationsState,
} from './agentRuns';
import type { SessionStatusEventDto } from '@/lib/tauri/commands';
import type { ExecutionEvent, Message } from '../types';

function event(overrides: Partial<ExecutionEvent>): ExecutionEvent {
  return {
    executionId: 'exec-1',
    workspaceId: 'w',
    taskId: 't',
    agentId: 'a',
    kind: 'started',
    message: 'Execution started',
    timestamp: 1,
    metadata: {},
    ...overrides,
  };
}

function message(overrides: Partial<Message>): Message {
  return {
    id: 'm1',
    workspaceId: 'w',
    agentId: 'a',
    executionId: 'exec-1',
    role: 'user',
    content: 'hi',
    timestamp: 1,
    failed: false,
    failureKind: null,
    ...overrides,
  };
}

function apply(...actions: ConversationAction[]): ConversationsState {
  return actions.reduce(conversationsReducer, initialConversations);
}

describe('conversationsReducer', () => {
  it('records the user message at once and marks the agent running', () => {
    const state = apply({
      type: 'sent',
      sent: { userMessage: message({}), executionId: 'exec-1' },
    });

    expect(state.messages['w/a']?.map((m) => m.content)).toEqual(['hi']);
    expect(state.runs['w/a']).toMatchObject({
      executionId: 'exec-1',
      status: 'running',
      activity: [],
    });
  });

  it('appends activity per event and finishes on a terminal event', () => {
    const state = apply(
      { type: 'sent', sent: { userMessage: message({}), executionId: 'exec-1' } },
      { type: 'event', event: event({ kind: 'started' }) },
      { type: 'event', event: event({ kind: 'sending_prompt', message: 'Sending prompt' }) },
      { type: 'event', event: event({ kind: 'completed', message: 'Execution completed' }) },
    );

    expect(state.runs['w/a']?.status).toBe('completed');
    expect(state.runs['w/a']?.activity.map((e) => e.kind)).toEqual([
      'started',
      'sending_prompt',
      'completed',
    ]);
  });

  it('keeps the run that events created before the send call returned', () => {
    const state = apply(
      { type: 'event', event: event({ kind: 'started' }) },
      { type: 'event', event: event({ kind: 'completed' }) },
      { type: 'sent', sent: { userMessage: message({}), executionId: 'exec-1' } },
    );

    expect(state.runs['w/a']?.status).toBe('completed');
    expect(state.runs['w/a']?.activity).toHaveLength(2);
  });

  it('keeps agents independent: one agent’s events never touch another', () => {
    const state = apply(
      {
        type: 'sent',
        sent: { userMessage: message({ id: 'm1', agentId: 'a' }), executionId: 'exec-a' },
      },
      {
        type: 'sent',
        sent: { userMessage: message({ id: 'm2', agentId: 'b' }), executionId: 'exec-b' },
      },
      {
        type: 'event',
        event: event({ agentId: 'a', executionId: 'exec-a', kind: 'waiting_for_model' }),
      },
      {
        type: 'event',
        event: event({
          agentId: 'b',
          executionId: 'exec-b',
          kind: 'failed',
          metadata: { failureKind: 'timeout' },
        }),
      },
    );

    expect(state.runs['w/a']).toMatchObject({ status: 'running', failureKind: null });
    expect(state.runs['w/a']?.activity).toHaveLength(1);
    expect(state.runs['w/b']).toMatchObject({ status: 'failed', failureKind: 'timeout' });
  });

  it('keeps the same agent in two workspaces independent', () => {
    const state = apply(
      {
        type: 'sent',
        sent: { userMessage: message({ id: 'm1', workspaceId: 'w1' }), executionId: 'exec-1' },
      },
      {
        type: 'sent',
        sent: { userMessage: message({ id: 'm2', workspaceId: 'w2' }), executionId: 'exec-2' },
      },
      {
        type: 'event',
        event: event({
          workspaceId: 'w1',
          executionId: 'exec-1',
          kind: 'failed',
          metadata: { failureKind: 'timeout' },
        }),
      },
      {
        type: 'event',
        event: event({
          workspaceId: 'w2',
          executionId: 'exec-2',
          kind: 'output_chunk',
          message: 'hello',
        }),
      },
    );

    expect(state.runs['w1/a']).toMatchObject({ status: 'failed', failureKind: 'timeout' });
    expect(state.runs['w2/a']).toMatchObject({ status: 'running', receiving: true });
    expect(state.streams['w1/a']).toBeUndefined();
    expect(state.streams['w2/a']?.text).toBe('hello');
    expect(state.messages['w1/a']).toHaveLength(1);
    expect(state.messages['w2/a']).toHaveLength(1);
  });

  it('remembers when a run started, for the "running for" clock', () => {
    const sent = apply({
      type: 'sent',
      sent: { userMessage: message({ timestamp: 1000 }), executionId: 'exec-1' },
    });
    const fromEvent = apply({ type: 'event', event: event({ timestamp: 5000 }) });

    expect(sent.runs['w/a']?.startedAt).toBe(1000);
    expect(fromEvent.runs['w/a']?.startedAt).toBe(5000);
  });

  it('ignores a late event of an older execution while a newer one runs', () => {
    const state = apply(
      { type: 'sent', sent: { userMessage: message({}), executionId: 'exec-2' } },
      { type: 'event', event: event({ executionId: 'exec-1', kind: 'completed' }) },
    );

    expect(state.runs['w/a']).toMatchObject({ executionId: 'exec-2', status: 'running' });
  });

  it('starts a new run when an agent that finished starts another execution', () => {
    const state = apply(
      { type: 'event', event: event({ executionId: 'exec-1', kind: 'completed' }) },
      { type: 'event', event: event({ executionId: 'exec-2', kind: 'started' }) },
    );

    expect(state.runs['w/a']).toMatchObject({ executionId: 'exec-2', status: 'running' });
    expect(state.runs['w/a']?.activity).toHaveLength(1);
  });

  it('adds messages once, in time order, per agent', () => {
    const answer = message({ id: 'm2', role: 'assistant', content: 'ok', timestamp: 5 });
    const state = apply(
      { type: 'message', message: answer },
      { type: 'loaded', messages: [message({ id: 'm1', timestamp: 2 }), answer] },
      { type: 'message', message: answer },
    );

    expect(state.messages['w/a']?.map((m) => m.id)).toEqual(['m1', 'm2']);
  });

  describe('live response', () => {
    const started = (): ConversationAction[] => [
      { type: 'sent', sent: { userMessage: message({}), executionId: 'exec-1' } },
      { type: 'event', event: event({ kind: 'waiting_for_model' }) },
    ];
    const chunk = (text: string, overrides: Partial<ExecutionEvent> = {}): ConversationAction => ({
      type: 'event',
      event: event({ kind: 'output_chunk', message: text, ...overrides }),
    });

    it('accumulates streamed text per agent and notes "Receiving response" once', () => {
      const state = apply(...started(), chunk('Reading '), chunk('the files.'));

      expect(state.streams['w/a']).toEqual({ executionId: 'exec-1', text: 'Reading the files.' });
      expect(state.runs['w/a']?.receiving).toBe(true);
      const kinds = state.runs['w/a']?.activity.map((e) => e.kind);
      expect(kinds?.filter((k) => k === 'output_chunk')).toHaveLength(1);
    });

    it('stops receiving when a tool starts and notes the next stretch of text again', () => {
      const state = apply(
        ...started(),
        chunk('Let me look.'),
        { type: 'event', event: event({ kind: 'tool_started', message: 'Using Glob' }) },
        chunk('Found it.'),
      );

      expect(state.streams['w/a']?.text).toBe('Let me look.Found it.');
      expect(state.runs['w/a']?.activity.filter((e) => e.kind === 'output_chunk')).toHaveLength(2);
    });

    it('keeps two agents’ streams apart', () => {
      const state = apply(
        { type: 'sent', sent: { userMessage: message({ agentId: 'a' }), executionId: 'exec-a' } },
        {
          type: 'sent',
          sent: { userMessage: message({ id: 'm2', agentId: 'b' }), executionId: 'exec-b' },
        },
        chunk('from A', { agentId: 'a', executionId: 'exec-a' }),
        chunk('from B', { agentId: 'b', executionId: 'exec-b' }),
      );

      expect(state.streams['w/a']?.text).toBe('from A');
      expect(state.streams['w/b']?.text).toBe('from B');
    });

    it('ignores streamed text from an execution the agent is not running', () => {
      const state = apply(...started(), chunk('stray', { executionId: 'exec-old' }));

      expect(state.streams['w/a']).toBeUndefined();
    });

    it('replaces the live text with the final message of the same execution', () => {
      const state = apply(
        ...started(),
        chunk('narration…'),
        { type: 'event', event: event({ kind: 'completed' }) },
        {
          type: 'message',
          message: message({ id: 'm2', role: 'assistant', content: 'Final', timestamp: 9 }),
        },
      );

      expect(state.streams['w/a']).toBeUndefined();
      expect(state.messages['w/a']?.at(-1)?.content).toBe('Final');
    });

    it('clears an old live text when the agent is sent a new message', () => {
      const state = apply(...started(), chunk('old'), {
        type: 'sent',
        sent: { userMessage: message({ id: 'm9' }), executionId: 'exec-2' },
      });

      expect(state.streams['w/a']).toBeUndefined();
    });
  });

  it('remembers a rejected send until the next send', () => {
    const failed = apply({ type: 'sendFailed', key: 'w/a', error: 'busy' });
    expect(failed.sendErrors['w/a']).toBe('busy');

    const retried = conversationsReducer(failed, {
      type: 'sent',
      sent: { userMessage: message({}), executionId: 'exec-1' },
    });
    expect(retried.sendErrors['w/a']).toBeUndefined();
  });

  describe('live process', () => {
    const status = (
      executionId: string,
      state: 'running' | 'interrupting' | 'terminating' | 'exited',
      extra: Partial<SessionStatusEventDto> = {},
    ): ConversationAction => ({
      type: 'processStatus',
      event: {
        executionId,
        processSessionId: `ps-${executionId}`,
        workspaceId: 'w',
        agentId: 'a',
        status: state,
        userAction: null,
        exitCode: null,
        timestamp: 10,
        ...extra,
      },
    });

    it('ends a run the user stopped as cancelled, not as failed', () => {
      const state = apply(
        { type: 'event', event: event({ kind: 'started' }) },
        { type: 'event', event: event({ kind: 'user_interrupted' }) },
        { type: 'event', event: event({ kind: 'cancelled' }) },
      );

      expect(state.runs['w/a']?.status).toBe('cancelled');
      expect(state.runs['w/a']?.failureKind).toBeNull();
      expect(state.runs['w/a']?.activity.map((a) => a.kind)).toEqual([
        'started',
        'user_interrupted',
        'cancelled',
      ]);
    });

    it('lets a new run start after a cancelled one', () => {
      const state = apply(
        { type: 'event', event: event({ kind: 'cancelled' }) },
        { type: 'event', event: event({ executionId: 'exec-2', kind: 'started' }) },
      );

      expect(state.runs['w/a']).toMatchObject({ executionId: 'exec-2', status: 'running' });
    });

    it('follows the process of each execution on its own', () => {
      const state = apply(
        status('exec-1', 'running'),
        status('exec-2', 'running'),
        status('exec-1', 'interrupting', { userAction: 'interrupted' }),
        status('exec-1', 'exited', { userAction: 'interrupted', exitCode: 130, timestamp: 50 }),
      );

      expect(state.processes['exec-1']).toMatchObject({
        status: 'exited',
        userAction: 'interrupted',
        exitCode: 130,
        startedAt: 10,
        endedAt: 50,
      });
      expect(state.processes['exec-2']).toMatchObject({ status: 'running', endedAt: null });
    });
  });

  describe('event ordering', () => {
    const at = (kind: ExecutionEvent['kind'], executionId = 'exec-1', extra = {}) =>
      ({ type: 'event', event: event({ kind, executionId, ...extra }) }) as const;

    it('does not take output that arrives after the end for a running answer', () => {
      const state = apply(
        at('started'),
        at('failed', 'exec-1', { metadata: { failureKind: 'timeout' } }),
        at('output_chunk', 'exec-1', { message: 'late text' }),
      );

      expect(state.runs['w/a']?.status).toBe('failed');
      expect(state.streams['w/a']).toBeUndefined();
      expect(state.runs['w/a']?.activity.map((e) => e.kind)).toEqual(['started', 'failed']);
    });

    it('keeps output that arrives while the execution runs', () => {
      const state = apply(
        at('started'),
        at('output_chunk', 'exec-1', { message: 'hel' }),
        at('output_chunk', 'exec-1', { message: 'lo' }),
      );

      expect(state.streams['w/a']?.text).toBe('hello');
    });

    it('keeps the first ending: a terminate then a failure is a failure, and nothing reopens it', () => {
      const state = apply(
        at('started'),
        at('user_terminated'),
        at('failed', 'exec-1', { metadata: { failureKind: 'execution_failed' } }),
        at('completed'),
        at('process_exited'),
      );

      const run = state.runs['w/a'];
      expect(run?.status).toBe('failed');
      expect(run?.failureKind).toBe('execution_failed');
    });

    it('does not bring back an older execution from a late event once another has run', () => {
      const state = apply(
        at('started', 'exec-1'),
        at('completed', 'exec-1'),
        at('started', 'exec-2'),
        at('completed', 'exec-2'),
        // A straggler of the first one, after the second has ended.
        at('process_exited', 'exec-1'),
        at('worktree_finalized', 'exec-1'),
      );

      expect(state.runs['w/a']?.executionId).toBe('exec-2');
      expect(state.runs['w/a']?.status).toBe('completed');
    });

    it('ignores a straggler of an older execution while a newer one runs', () => {
      const state = apply(
        at('started', 'exec-1'),
        at('completed', 'exec-1'),
        at('started', 'exec-2'),
        at('output_chunk', 'exec-1', { message: 'old' }),
        at('failed', 'exec-1'),
      );

      expect(state.runs['w/a']?.executionId).toBe('exec-2');
      expect(state.runs['w/a']?.status).toBe('running');
      expect(state.streams['w/a']).toBeUndefined();
    });

    it('still picks up an execution that was already running when the app opened', () => {
      const state = apply(at('permission', 'exec-7'));

      expect(state.runs['w/a']?.executionId).toBe('exec-7');
      expect(state.runs['w/a']?.status).toBe('running');
    });

    it('shows a worktree event after completion as part of the same, finished execution', () => {
      const state = apply(at('started'), at('completed'), at('worktree_finalized'));

      expect(state.runs['w/a']?.status).toBe('completed');
      expect(state.runs['w/a']?.activity.map((e) => e.kind)).toEqual([
        'started',
        'completed',
        'worktree_finalized',
      ]);
    });
  });
});
