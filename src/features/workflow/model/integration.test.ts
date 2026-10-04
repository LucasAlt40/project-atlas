import {
  changeSet,
  handoff,
  nodeState,
  passwordRecovery,
  run,
  withCode,
} from '@/test/workflowFixtures';
import type { IntegrationStatusDto } from '@/lib/tauri/commands';
import { deliveryOf, handoffsOf, handoffsOver } from './integration';

const done = () => run(passwordRecovery(), 'completed', { architect: nodeState('completed') });

describe('what the screen may say about the code', () => {
  it('says the code is in the project only when the changes were applied', () => {
    const statuses: IntegrationStatusDto[] = [
      'not_applicable',
      'in_progress',
      'no_changes',
      'changes_available',
      'conflicts',
      'blocked',
      'integrated',
      'kept_isolated',
      'discarded',
      'failed',
    ];
    for (const status of statuses) {
      const delivery = deliveryOf(withCode(done(), status));
      expect(delivery?.inProject ?? false, status).toBe(status === 'integrated');
      // No other state ever uses the sentence that says so.
      if (status !== 'integrated') {
        expect(delivery?.codeLine, status).not.toBe('integration.line.integrated');
      }
    }
  });

  it('offers the decisions that make sense for where the code is', () => {
    const offers = (
      status: IntegrationStatusDto,
      extra = {},
      runStatus: 'completed' | 'cancelled' = 'completed',
    ) => {
      const base = runStatus === 'completed' ? done() : { ...done(), status: runStatus };
      const d = deliveryOf(withCode(base, status, extra));
      return d && [d.review, d.openInIde, d.apply, d.keep, d.discard];
    };
    expect(offers('changes_available')).toEqual([true, true, true, true, true]);
    // A cancelled run's work can be looked at, kept, opened or dropped, never applied.
    expect(
      offers(
        'changes_available',
        { canApply: false, blockReason: 'execution_not_completed' },
        'cancelled',
      ),
    ).toEqual([true, true, false, true, true]);
    expect(offers('integrated')).toEqual([true, true, false, false, false]);
    expect(offers('no_changes')).toEqual([false, false, false, false, false]);
    expect(offers('discarded')).toEqual([false, false, false, false, false]);
    expect(offers('kept_isolated')).toEqual([true, true, true, false, true]);
    expect(offers('blocked', { blockReason: 'policy_denied', canApply: false })).toEqual([
      true,
      true,
      false,
      true,
      true,
    ]);
    expect(offers('blocked', { blockReason: 'base_dirty', canApply: true })?.[2]).toBe(true);
  });

  it('keeps how the run ended apart from where the code is', () => {
    const failed = { ...done(), status: 'failed' as const };
    const delivery = deliveryOf(withCode(failed, 'changes_available', { canApply: false }));
    expect(delivery?.runLine).toBe('integration.run.failed');
    expect(delivery?.codeLine).toBe('integration.line.unfinished');
    expect(deliveryOf({ ...done(), status: 'running' })).toBeNull();
    expect(deliveryOf(done())).toBeNull();
  });

  it('words each block reason on its own', () => {
    const lines = new Set(
      (
        [
          'policy_denied',
          'base_dirty',
          'base_branch_changed',
          'uncommitted_changes',
          'validation_failed',
        ] as const
      ).map((blockReason) => deliveryOf(withCode(done(), 'blocked', { blockReason }))?.codeLine),
    );
    expect(lines.size).toBe(5);
  });
});

describe('handoffs of a run', () => {
  const r = {
    ...done(),
    handoffs: [
      handoff('architect', 'developer'),
      handoff('developer', 'qa', { iteration: 1 }),
      handoff('developer', 'qa', { iteration: 2, id: 'again' }),
    ],
  };

  it('finds what travelled over a connection, oldest first', () => {
    expect(handoffsOver(r, 'developer->qa').map((h) => h.iteration)).toEqual([1, 2]);
    expect(handoffsOver(r, 'nothing')).toEqual([]);
  });

  it('finds what a node received and what it sent', () => {
    const developer = handoffsOf(r, 'developer');
    expect(developer.received.map((h) => h.fromNodeId)).toEqual(['architect']);
    expect(developer.sent).toHaveLength(2);
    expect(handoffsOf(r, 'architect').received).toEqual([]);
    expect(changeSet().files).toHaveLength(2);
  });
});
