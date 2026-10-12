import {
  importPreview,
  modelAssessment,
  previewItem,
  externalItem,
} from '@/test/portabilityFixtures';
import {
  asksToEdit,
  blockers,
  initialChoices,
  summarize,
  toRequest,
  uniqueBlockers,
  without,
} from './importPlan';

const personality = previewItem('personality-1', 'personality', 'Reviewer', { root: false });
const agent = previewItem('agent-2', 'agent', 'Architect', {
  dependsOn: ['personality-1'],
  model: modelAssessment('available'),
});

describe('the import plan', () => {
  it('starts from what the core proposes and pre-selects, without applying, a same-name match', () => {
    const preview = importPreview([personality, agent], {
      external: [externalItem('agent-9', 'agent', 'Helper', { suggested: 'agent-local' })],
    });

    const choices = initialChoices(preview, 'ws-1');

    expect(choices.decisions).toEqual({ 'personality-1': 'create', 'agent-2': 'create' });
    expect(choices.bindings).toEqual({ 'agent-9': 'agent-local' });
    expect(choices.keepPermissions).toBe(false);
    expect(choices.workspaceId).toBe('ws-1');
  });

  it('lets a file with nothing in conflict be imported as it stands', () => {
    const preview = importPreview([personality, agent]);

    expect(blockers(preview, initialChoices(preview, null))).toEqual([]);
    expect(summarize(preview, initialChoices(preview, null))).toEqual({
      create: 2,
      replace: 0,
      reuse: 0,
      skip: 0,
    });
  });

  it('wants a workspace for workflows and says nothing about one for the rest', () => {
    const workflow = previewItem('workflow-1', 'workflow', 'Flow');
    const withWorkflow = importPreview([workflow]);

    expect(blockers(withWorkflow, initialChoices(withWorkflow, null))).toEqual([
      { code: 'workspace_required' },
    ]);
    expect(blockers(withWorkflow, initialChoices(withWorkflow, 'ws-1'))).toEqual([]);
    const without = importPreview([personality]);
    expect(blockers(without, initialChoices(without, null))).toEqual([]);
  });

  it('refuses to import when everything is left out or reused', () => {
    const preview = importPreview([personality]);
    const choices = initialChoices(preview, null);
    choices.decisions['personality-1'] = 'skip';

    expect(blockers(preview, choices)).toEqual([{ code: 'nothing_to_do' }]);
    choices.decisions['personality-1'] = 'use_existing';
    expect(blockers(preview, choices)).toEqual([{ code: 'nothing_to_do' }]);
  });

  it('wants a model chosen for an agent whose runtime cannot be kept, until one is', () => {
    const lost = previewItem('agent-2', 'agent', 'Architect', {
      model: modelAssessment('runtime_missing'),
    });
    const preview = importPreview([lost]);
    const choices = initialChoices(preview, null);

    expect(blockers(preview, choices)).toEqual([{ code: 'model_choice_required', ref: 'agent-2' }]);
    choices.models['agent-2'] = { runtimeId: 'opencode', modelId: 'opencode/alt' };
    expect(blockers(preview, choices)).toEqual([]);
  });

  it('does not ask for a model of an agent that is not going to be written', () => {
    const lost = previewItem('agent-2', 'agent', 'Architect', {
      model: modelAssessment('runtime_missing'),
    });
    const preview = importPreview([lost, personality]);
    const choices = initialChoices(preview, null);
    choices.decisions['agent-2'] = 'skip';

    expect(blockers(preview, choices)).toEqual([]);
  });

  it('wants a replacement confirmed and forgets the confirmation when the decision changes', () => {
    const conflicting = previewItem('agent-2', 'agent', 'Architect', {
      existing: { id: 'a', name: 'Architect' },
      allowed: ['skip', 'create', 'use_existing', 'replace'],
    });
    const preview = importPreview([conflicting]);
    const choices = initialChoices(preview, null);
    choices.decisions['agent-2'] = 'replace';

    expect(blockers(preview, choices)).toEqual([{ code: 'replace_unconfirmed', ref: 'agent-2' }]);
    choices.confirmedReplacements = ['agent-2'];
    expect(blockers(preview, choices)).toEqual([]);
    expect(summarize(preview, choices).replace).toBe(1);
  });

  it('stops what depends on something that is left out, and on what the file does not carry', () => {
    const preview = importPreview([personality, agent], {
      external: [externalItem('agent-9', 'agent', 'Helper')],
    });
    const choices = initialChoices(preview, null);
    choices.decisions['personality-1'] = 'skip';
    expect(blockers(preview, choices)).toEqual([{ code: 'dependency_skipped', ref: 'agent-2' }]);

    const bare = importPreview(
      [previewItem('workflow-1', 'workflow', 'Flow', { dependsOn: ['agent-9'] })],
      {
        external: [externalItem('agent-9', 'agent', 'Helper')],
      },
    );
    const bareChoices = initialChoices(bare, 'ws-1');
    expect(blockers(bare, bareChoices)).toEqual([{ code: 'external_unbound', ref: 'agent-9' }]);
    bareChoices.bindings['agent-9'] = 'agent-local';
    expect(blockers(bare, bareChoices)).toEqual([]);
  });

  it('reuses what is here without it counting as an import', () => {
    const preview = importPreview([personality, agent]);
    const choices = initialChoices(preview, null);
    choices.decisions['personality-1'] = 'use_existing';

    expect(blockers(preview, choices)).toEqual([]);
    expect(summarize(preview, choices)).toEqual({ create: 1, replace: 0, reuse: 1, skip: 0 });
  });

  it('sends only what the core would accept: no model for what is not written, no stray bindings', () => {
    const preview = importPreview([personality, agent], {
      external: [externalItem('agent-9', 'agent', 'Helper')],
    });
    const choices = initialChoices(preview, 'ws-1');
    choices.decisions['agent-2'] = 'skip';
    choices.models['agent-2'] = { runtimeId: 'opencode', modelId: 'm' };
    choices.bindings['agent-9'] = 'local';
    choices.bindings['agent-404'] = 'stale';
    choices.keepPermissions = true;

    expect(toRequest(preview, choices)).toEqual({
      workspaceId: 'ws-1',
      decisions: { 'personality-1': 'create', 'agent-2': 'skip' },
      models: {},
      bindings: { 'agent-9': 'local' },
      keepPermissionProfiles: true,
    });
  });

  it('offers the permission question only when an agent that is going in asks to edit', () => {
    const editing = previewItem('agent-2', 'agent', 'Architect', {
      issues: [{ code: 'grants_editing', severity: 'warning', params: {} }],
    });
    const preview = importPreview([editing]);
    const choices = initialChoices(preview, null);

    expect(asksToEdit(preview, choices)).toBe(true);
    choices.decisions['agent-2'] = 'skip';
    expect(asksToEdit(preview, choices)).toBe(false);
  });

  it('names a blocker once however many items raise it', () => {
    expect(
      uniqueBlockers([
        { code: 'external_unbound', ref: 'a' },
        { code: 'external_unbound', ref: 'a' },
        { code: 'external_unbound', ref: 'b' },
      ]),
    ).toHaveLength(2);
    expect(without({ a: 1, b: 2 }, 'a')).toEqual({ b: 2 });
  });
});
