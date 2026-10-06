import { createTranslator } from '@/i18n';
import { toActivityEntry, activityLabel } from './activity';
import { applyApprovalEvent } from './approvals';
import { conversationsReducer, initialConversations } from './agentRuns';
import { agentStatus } from './agentStatus';
import type { ExecutionEvent } from '../types';

const permission = (metadata: Record<string, string>): ExecutionEvent => ({
  executionId: 'e1',
  workspaceId: 'w1',
  taskId: 't',
  agentId: 'a1',
  kind: 'permission',
  message: '',
  timestamp: 1,
  metadata,
});

describe('permission events', () => {
  it.each(['pt-BR', 'en-US'] as const)(
    'are worded in %s for every decision and reason, never as a raw key',
    (language) => {
      const t = createTranslator(language);
      const decisions = [
        'allowed',
        'denied',
        'approval_requested',
        'approved',
        'rejected',
        'transformed',
      ];
      for (const decision of decisions) {
        for (const action of [
          'launch_runtime',
          'run_process',
          'review_context',
          'edit_files',
          'share_result',
        ]) {
          const label = activityLabel(
            t,
            toActivityEntry(
              permission({ decision, action, target: 'npm test', reason: 'outside_project' }),
              0,
            ),
          );
          expect(label).toContain('npm test');
          expect(label).not.toMatch(/^(agent|permission)\./);
        }
      }
    },
  );

  it('does not break on a reason code the UI does not know yet', () => {
    const t = createTranslator('en-US');
    const entry = toActivityEntry(
      permission({ decision: 'denied', action: 'run_process', target: 'x', reason: 'brand_new' }),
      0,
    );

    expect(() => activityLabel(t, entry)).not.toThrow();
  });

  it('puts the agent in "waiting for approval" until the user answers, using real events', () => {
    const request = permission({
      decision: 'approval_requested',
      action: 'run_process',
      target: 'make all',
      approvalId: 'approval-1',
      cwd: '/p',
      reason: 'not_in_allowed_list',
    });
    let state = conversationsReducer(initialConversations, { type: 'event', event: request });

    expect(agentStatus(state.runs['w1/a1'], undefined)).toBe('waiting_approval');
    expect(state.approvals).toHaveLength(1);
    expect(state.approvals[0]).toMatchObject({ command: 'make all', cwd: '/p' });

    state = conversationsReducer(state, {
      type: 'event',
      event: permission({ decision: 'approved', approvalId: 'approval-1', target: 'make all' }),
    });

    expect(agentStatus(state.runs['w1/a1'], undefined)).toBe('running');
    expect(state.approvals).toHaveLength(0);
    expect(applyApprovalEvent([], request)).toHaveLength(1);
  });
});
