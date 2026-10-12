import { invokeCommand } from '@/lib/tauri/commands';
import { startWorkflow } from '@/features/workflow/services/workflowService';
import {
  analyzeProject,
  getProjectHarness,
  initializeProject,
  initializeWorkspaceHarness,
  refreshProjectHarness,
  sendMessage,
} from './workspaceService';

vi.mock('@/lib/tauri/commands', async (importOriginal) => ({
  ...(await importOriginal<Record<string, unknown>>()),
  invokeCommand: vi.fn(() => Promise.resolve(null)),
}));

const invoked = () => vi.mocked(invokeCommand).mock.calls.at(-1) as unknown[];

describe('what the UI asks the core about projects', () => {
  beforeEach(() => {
    vi.mocked(invokeCommand).mockClear();
  });

  it('sends exactly what it always sent when no project is named', async () => {
    await analyzeProject('w1');
    expect(invoked()).toEqual(['analyze_project', { workspaceId: 'w1' }]);
    await getProjectHarness('w1');
    expect(invoked()).toEqual(['get_project_harness', { workspaceId: 'w1' }]);
    await refreshProjectHarness('w1', true);
    expect(invoked()).toEqual(['refresh_project_harness', { workspaceId: 'w1', confirm: true }]);
    await sendMessage({ workspaceId: 'w1', agentId: 'a1', content: 'hi' });
    expect(invoked()).toEqual([
      'send_message',
      { request: { workspaceId: 'w1', agentId: 'a1', content: 'hi' } },
    ]);
    await startWorkflow('wf', 'task');
    expect(invoked()).toEqual(['start_workflow', { workflowId: 'wf', task: 'task' }]);
  });

  it('names the project when one is chosen', async () => {
    await analyzeProject('w1', undefined, 'p2');
    expect(invoked()).toEqual(['analyze_project', { workspaceId: 'w1', projectId: 'p2' }]);
    await getProjectHarness('w1', 'p2');
    expect(invoked()).toEqual(['get_project_harness', { workspaceId: 'w1', projectId: 'p2' }]);
    await refreshProjectHarness('w1', false, 'p2');
    expect(invoked()).toEqual([
      'refresh_project_harness',
      { workspaceId: 'w1', projectId: 'p2', confirm: false },
    ]);
    const input = { mode: 'create' } as Parameters<typeof initializeProject>[1];
    await initializeProject('w1', input, 'p2');
    expect(invoked()).toEqual([
      'initialize_project',
      { workspaceId: 'w1', projectId: 'p2', input },
    ]);
  });

  it('hands a scope to a message and to a workflow run, and only then', async () => {
    const scope = { workingProjectId: 'p2', readProjectIds: ['p1'] };

    await sendMessage({ workspaceId: 'w1', agentId: 'a1', content: 'hi', scope });
    expect(invoked()).toEqual([
      'send_message',
      { request: { workspaceId: 'w1', agentId: 'a1', content: 'hi', scope } },
    ]);
    await startWorkflow('wf', 'task', true, scope);
    expect(invoked()).toEqual([
      'start_workflow',
      { workflowId: 'wf', task: 'task', autoApprovePlans: true, scope },
    ]);
  });

  it('starts the Harness of every project, or of the ones named', async () => {
    await initializeWorkspaceHarness('w1');
    expect(invoked()).toEqual(['initialize_workspace_harness', { workspaceId: 'w1' }]);
    await initializeWorkspaceHarness('w1', ['p1', 'p3']);
    expect(invoked()).toEqual([
      'initialize_workspace_harness',
      { workspaceId: 'w1', projectIds: ['p1', 'p3'] },
    ]);
  });
});
