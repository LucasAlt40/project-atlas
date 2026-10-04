import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { I18nProvider } from '@/i18n/I18nProvider';
import { storedExecution } from '@/test/fixtures';
import { ExecutionInspector } from './ExecutionInspector';

const context = {
  workspaceName: 'ERP',
  agentName: 'Architect',
  personalityName: 'Architect',
  runtimeName: 'Claude Code',
  capabilities: undefined,
  worktreeIsolation: true,
};

const link = {
  workflowId: 'wf-1',
  workflowName: 'Password recovery',
  workflowExecutionId: 'wfx-1',
  nodeId: 'backend',
  nodeLabel: 'Backend',
  attempt: 2,
  iteration: 1,
};

function show(workflow: typeof link | undefined, onOpenWorkflow?: () => void) {
  const execution = storedExecution('w1', 'a1', 'exec-42', workflow ? { workflow } : {});
  render(
    <I18nProvider language="en-US">
      <ExecutionInspector
        execution={execution}
        messages={[]}
        context={context}
        onClose={vi.fn()}
        {...(onOpenWorkflow ? { onOpenWorkflow } : {})}
      />
    </I18nProvider>,
  );
}

describe('Execution Inspector breadcrumb', () => {
  it('says where a workflow step sits: workflow, name, node, execution', () => {
    show(link);

    const crumbs = screen.getByRole('navigation', { name: 'Where this execution sits' });
    const items = within(crumbs)
      .getAllByRole('listitem')
      .map((item) => item.textContent);
    expect(items).toEqual([
      'Workflow',
      'Password recovery',
      'Backend (attempt 2)',
      'Execution #42',
    ]);
  });

  it('takes the user to the workflow run when asked to', async () => {
    const onOpenWorkflow = vi.fn();
    show(link, onOpenWorkflow);

    await userEvent.click(screen.getByRole('button', { name: 'Workflow' }));

    expect(onOpenWorkflow).toHaveBeenCalledWith(link);
  });

  it('has no breadcrumb for an execution that is not a workflow step', () => {
    show(undefined);

    expect(screen.queryByRole('navigation', { name: 'Where this execution sits' })).toBeNull();
    expect(screen.getByRole('heading', { name: 'Execution #42' })).toBeInTheDocument();
  });
});
