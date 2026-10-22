import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { I18nProvider } from '@/i18n/I18nProvider';
import type { ToolPlanItemDto, WorkflowTemplateDto } from '@/lib/tauri/commands';
import { basicTemplate, templates } from '@/test/workflowFixtures';
import { NewWorkflowPanel } from './NewWorkflowPanel';

const ready: ToolPlanItemDto = {
  nodeId: 'architect',
  purpose: 'Read the documentation of a library the design relies on.',
  capability: 'docs',
  connectionId: 'c-docs',
  connection: 'context7',
  tools: { kind: 'only', tools: ['get-library-docs'] },
  reason: null,
  granted: false,
};
const missing: ToolPlanItemDto = {
  nodeId: 'qa',
  purpose: 'Open the page to see the behaviour.',
  capability: 'browser',
  connectionId: null,
  connection: null,
  tools: null,
  reason: { kind: 'no_connection' },
  granted: false,
};

const saved: WorkflowTemplateDto = {
  ...basicTemplate,
  id: 'template-1',
  kind: 'custom',
  name: 'House rules',
  basedOn: 'feature_basic',
};

function show(plan: ToolPlanItemDto[], list: WorkflowTemplateDto[] = [...templates, saved]) {
  const onCreate = vi.fn();
  const onDeleteTemplate = vi.fn();
  const planTools = vi.fn(() => Promise.resolve(plan));
  render(
    <I18nProvider language="en-US">
      <NewWorkflowPanel
        templates={list}
        recommend={() => Promise.resolve('software_feature')}
        planTools={planTools}
        agentName={(id) => `agent ${id}`}
        onDeleteTemplate={onDeleteTemplate}
        error={null}
        onCreate={onCreate}
      />
    </I18nProvider>,
  );
  return { onCreate, onDeleteTemplate, planTools };
}


/** The form is in steps: Task, then Template, then Name. */
async function next(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('button', { name: 'Continue' }));
}

describe('choosing a template', () => {
  it('shows what the recommended template will do before anything is created', async () => {
    const user = userEvent.setup();
    show([ready, missing]);
    await next(user);

    const details = await screen.findByRole('region', { name: 'Template details' });

    expect(within(details).getByText('Implement')).toBeInTheDocument();
    expect(within(details).getByText('Built-in')).toBeInTheDocument();
    expect(
      within(details).getByText(/Turn a feature request into a reviewed change/),
    ).toBeInTheDocument();
    // Each step says who does it, whether it may edit, and when it is done.
    expect(within(details).getByText('Analyze and plan')).toBeInTheDocument();
    const qa = within(details).getByText('Check behaviour').closest('details') as HTMLElement;
    expect(within(qa).getByText('Read-only')).toBeInTheDocument();
    expect(within(qa).getByText(/repeats up to 3 times/)).toBeInTheDocument();
    expect(within(qa).getByText('Edge cases were traced.')).toBeInTheDocument();
    expect(within(details).getByText('A plan sized to the task')).toBeInTheDocument();
  });

  it('says which tool each step would get here and why a step would get none', async () => {
    const user = userEvent.setup();
    show([ready, missing]);
    await next(user);

    const details = await screen.findByRole('region', { name: 'Template details' });

    expect(
      await within(details).findByText('Documentation: context7 (only: get-library-docs)'),
    ).toBeInTheDocument();
    expect(
      within(details).getByText('Browser: no connection in this workspace'),
    ).toBeInTheDocument();
    // A suggestion authorizes nothing: it is said, and the option to apply it is the person's.
    expect(within(details).getByText(/authorizes nothing by itself/)).toBeInTheDocument();
  });

  it('creates with the suggested tools only when there is one to give and the person keeps the option', async () => {
    const user = userEvent.setup();
    const { onCreate } = show([ready, missing]);
    await user.type(screen.getByLabelText('Task'), 'Add a toggle');
    await next(user);
    const box = await screen.findByRole('checkbox', {
      name: /Give each step the tools suggested for it/,
    });
    await screen.findByText(/context7/);
    expect(box).toBeChecked();

    await next(user);
    await user.click(screen.getByRole('button', { name: 'Create workflow' }));
    expect(onCreate).toHaveBeenLastCalledWith(
      expect.objectContaining({ templateId: 'software_feature', applyTools: true }),
    );

    await user.click(screen.getByRole('button', { name: 'Back' }));
    await user.click(
      screen.getByRole('checkbox', { name: /Give each step the tools suggested for it/ }),
    );
    await next(user);
    await user.click(screen.getByRole('button', { name: 'Create workflow' }));
    expect(onCreate).toHaveBeenLastCalledWith(expect.objectContaining({ applyTools: false }));
  });

  it('offers no grant at all when the workspace can give nothing', async () => {
    const user = userEvent.setup();
    const { onCreate } = show([missing]);
    await user.type(screen.getByLabelText('Task'), 'Add a toggle');
    await next(user);
    const box = await screen.findByRole('checkbox', {
      name: /Give each step the tools suggested for it/,
    });
    await screen.findByText(/no connection in this workspace/);

    expect(box).toBeDisabled();
    await next(user);
    await user.click(screen.getByRole('button', { name: 'Create workflow' }));
    expect(onCreate).toHaveBeenLastCalledWith(expect.objectContaining({ applyTools: false }));
  });

  it('shows the built-in templates and the ones the person saved as cards, and only those can be deleted', async () => {
    const user = userEvent.setup();
    const { onDeleteTemplate } = show([]);
    await user.click(await screen.findByRole('radio', { name: /Custom/ }));
    await next(user);

    // One card each, with what is really known of it; a saved one says so.
    const cards = screen
      .getAllByRole('radio')
      .filter((r) => r.getAttribute('name') === 'workflow-template');
    expect(cards.map((c) => c.getAttribute('value'))).toEqual([
      'software_feature',
      'feature_basic',
      'bug_fix',
      'code_review',
      'template-1',
      '',
    ]);
    await user.click(screen.getByRole('radio', { name: /^Implement/ }));
    expect(screen.queryByRole('button', { name: 'Delete template' })).not.toBeInTheDocument();
    expect(screen.getByText('2 steps · 2 suggested tools')).toBeInTheDocument();

    await user.click(screen.getByRole('radio', { name: /House rules/ }));
    const details = screen.getByRole('region', { name: 'Template details' });
    expect(within(details).getByText('Yours')).toBeInTheDocument();
    await user.click(within(details).getByRole('button', { name: 'Delete template' }));

    expect(onDeleteTemplate).toHaveBeenCalledWith('template-1');
  });

  it('in Automatic mode follows the recommendation and always lets the person pick another workflow', async () => {
    const user = userEvent.setup();
    const { onCreate } = show([]);
    await user.type(screen.getByLabelText('Task'), 'Fix the login');
    await next(user);
    // The recommendation (the fixture proposes the Implementer) is the one chosen, and marked.
    const recommended = await screen.findByRole('radio', { name: /^Implement/ });
    await waitFor(() => {
      expect(recommended).toBeChecked();
    });
    expect(screen.getByText('Recommended')).toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('Suggested for your task: Implement');

    // Another workflow is one click away, and what is shown and created follows it.
    await user.click(screen.getByRole('radio', { name: /Debug & fix/ }));
    expect(screen.getByRole('status')).toHaveTextContent('Workflow chosen by you: Debug & fix');
    await next(user);
    await user.click(screen.getByRole('button', { name: 'Create workflow' }));
    expect(onCreate).toHaveBeenLastCalledWith(
      expect.objectContaining({ templateId: 'bug_fix', mode: 'automatic' }),
    );

    // Going back to the recommended one follows the recommendation again.
    await user.click(screen.getByRole('button', { name: 'Back' }));
    await user.click(screen.getByRole('radio', { name: /^Implement/ }));
    expect(screen.getByRole('status')).toHaveTextContent('Suggested for your task');
  });
});
