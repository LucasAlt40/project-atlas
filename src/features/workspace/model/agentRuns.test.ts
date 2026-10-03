import {
  conversationsReducer,
  initialConversations,
  type ConversationAction,
  type ConversationsState,
} from './agentRuns';
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
});
