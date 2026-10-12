import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { I18nProvider } from '@/i18n/I18nProvider';
import type { OutcomeDto } from '@/lib/tauri/commands';
import { agent } from '@/test/fixtures';
import { handoff, nodeState, passwordRecovery, run } from '@/test/workflowFixtures';
import type { Workflow, WorkflowEdge } from '../types';
import { HandoffView } from './HandoffView';
import { EdgeInspector, NodeInspector } from './WorkflowInspector';

const PASS_FAIL: OutcomeDto[] = [
  { id: 'pass', label: 'Pass', description: 'Validation passed.' },
  { id: 'fail', label: 'Fail', description: 'Validation failed.' },
];

function connection(value: string | null): { workflow: Workflow; edge: WorkflowEdge } {
  const workflow = passwordRecovery();
  const edge: WorkflowEdge = {
    id: 'qa->bug-fixer',
    sourceNodeId: 'qa',
    targetNodeId: 'bug-fixer',
    condition: value === null ? null : { field: 'result.outcome', operator: 'equals', value },
    label: '',
  };
  return { workflow: { ...workflow, edges: [edge] }, edge };
}

function inspect(
  edge: WorkflowEdge,
  workflow: Workflow,
  outcomes: OutcomeDto[],
  onChange = vi.fn(),
) {
  render(
    <I18nProvider language="en-US">
      <EdgeInspector
        workflow={workflow}
        edge={edge}
        editable
        outcomesOf={() => outcomes}
        agentName={() => 'Architecture Validator'}
        onChange={onChange}
      />
    </I18nProvider>,
  );
  return onChange;
}

describe('the connection editor', () => {
  it('offers exactly the outcomes the source agent declares', () => {
    const { workflow, edge } = connection(null);
    inspect(edge, workflow, PASS_FAIL);

    const when = screen.getByLabelText('Taken when');
    expect(
      within(when)
        .getAllByRole('option')
        .map((o) => o.textContent),
    ).toEqual(['Always', 'Pass (pass)', 'Fail (fail)', 'A condition…']);
  });

  it('turns a chosen outcome into a condition on result.outcome', async () => {
    const user = userEvent.setup();
    const { workflow, edge } = connection(null);
    const onChange = inspect(edge, workflow, PASS_FAIL);

    await user.selectOptions(screen.getByLabelText('Taken when'), 'outcome:fail');

    const change = onChange.mock.calls[0]?.[0] as (w: Workflow) => Workflow;
    expect(change(workflow).edges[0]).toMatchObject({
      condition: { field: 'result.outcome', operator: 'equals', value: 'fail' },
      label: 'Fail',
    });
  });

  it('shows a declared outcome as chosen, and says when an outcome is not declared', () => {
    const declared = connection('fail');
    inspect(declared.edge, declared.workflow, PASS_FAIL);
    expect(screen.getByLabelText('Taken when')).toHaveValue('outcome:fail');
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('flags a condition on an outcome the agent never declares, before any run', () => {
    const { workflow, edge } = connection('success');
    inspect(edge, workflow, PASS_FAIL);

    expect(screen.getByLabelText('Taken when')).toHaveValue('custom');
    expect(screen.getByRole('alert')).toHaveTextContent(
      '"success" is not an outcome declared by the agent "Architecture Validator".',
    );
    // The value can only be picked among the declared ones.
    const value = screen.getByLabelText('Value');
    expect(
      within(value)
        .getAllByRole('option')
        .map((o) => o.getAttribute('value')),
    ).toEqual(['', 'pass', 'fail', 'success']);
  });

  it('keeps the older pass/fail choices for an agent that declares no outcome', () => {
    const { workflow, edge } = connection(null);
    inspect(edge, workflow, []);

    expect(
      within(screen.getByLabelText('Taken when'))
        .getAllByRole('option')
        .map((o) => o.getAttribute('value')),
    ).toEqual(['always', 'pass', 'fail', 'custom']);
  });
});

describe('a handoff', () => {
  it('says the execution completed even when the outcome is fail, and lists findings with their place', () => {
    const workflow = passwordRecovery();
    const result = run(workflow, 'running', { qa: nodeState('completed') });
    render(
      <I18nProvider language="en-US">
        <HandoffView
          run={result}
          onOpenExecution={vi.fn()}
          handoff={handoff('qa', 'bug-fixer', {
            status: 'fail',
            outcome: 'fail',
            summary: 'The layering is broken.',
            validation: {
              status: 'fail',
              outcome: 'fail',
              summary: 'The layering is broken.',
              findings: [
                {
                  severity: 'high',
                  category: 'architecture',
                  title: 'Domain imports infra',
                  file: 'src/domain/a.rs',
                  line: 12,
                  description: 'The domain depends on infrastructure.',
                  evidence: '',
                  recommendation: '',
                },
              ],
            },
          })}
        />
      </I18nProvider>,
    );

    expect(screen.getByText('Execution:', { exact: false })).toHaveTextContent(
      'Execution: completed · Outcome: FAIL',
    );
    const finding = screen.getByText(/Domain imports infra/).closest('li');
    expect(finding).toHaveTextContent('src/domain/a.rs:12');
    expect(finding).toHaveTextContent('The domain depends on infrastructure.');
  });

  it('says a failed step failed, with no outcome', () => {
    const workflow = passwordRecovery();
    render(
      <I18nProvider language="en-US">
        <HandoffView
          run={run(workflow, 'running', {})}
          onOpenExecution={vi.fn()}
          handoff={handoff('qa', 'bug-fixer', {
            kind: 'failure',
            status: 'fail',
            failure: 'No valid outcome: the agent did not declare one of [pass, fail]',
          })}
        />
      </I18nProvider>,
    );

    expect(screen.getByText('Execution:', { exact: false })).toHaveTextContent('Execution: failed');
    expect(screen.queryByText(/Outcome:/)).not.toBeInTheDocument();
  });
});

describe('the integrations setting of an agent step', () => {
  function show(onChange = vi.fn()) {
    const workflow = passwordRecovery();
    const node = workflow.nodes.find((n) => n.type === 'agent');
    if (node?.type !== 'agent') throw new Error('no agent node');
    render(
      <I18nProvider language="en-US">
        <NodeInspector
          workflow={workflow}
          node={node}
          editable
          agents={[agent(node.agentId, 'Dev')]}
          factsOf={() => undefined}
          run={undefined}
          onChange={onChange}
          onOpenExecution={() => undefined}
        />
      </I18nProvider>,
    );
    return { onChange, node, workflow };
  }

  it('is on by default and turning it off changes only that setting of the step', async () => {
    const { onChange, node, workflow } = show();
    const box = screen.getByRole('checkbox', {
      name: /Tell the agent which integrations it was given/,
    });
    expect(box).toBeChecked();

    await userEvent.click(box);

    const change = onChange.mock.calls[0]?.[0] as (w: Workflow) => Workflow;
    const changed = change(workflow).nodes.find((n) => n.id === node.id);
    expect(changed?.type === 'agent' && changed.executionPolicy).toEqual({
      isolation: 'from_agent',
      announceIntegrations: false,
      readOnly: false,
    });
  });
});

describe('the completion criteria and the read-only setting of an agent step', () => {
  function show(onChange = vi.fn()) {
    const workflow = passwordRecovery();
    const node = workflow.nodes.find((n) => n.type === 'agent');
    if (node?.type !== 'agent') throw new Error('no agent node');
    render(
      <I18nProvider language="en-US">
        <NodeInspector
          workflow={workflow}
          node={node}
          editable
          agents={[agent(node.agentId, 'Dev')]}
          factsOf={() => undefined}
          run={undefined}
          onChange={onChange}
          onOpenExecution={() => undefined}
        />
      </I18nProvider>,
    );
    return { onChange, node, workflow };
  }
  const changed = (onChange: ReturnType<typeof vi.fn>, workflow: Workflow, id: string) => {
    const change = onChange.mock.calls.at(-1)?.[0] as (w: Workflow) => Workflow;
    return change(workflow).nodes.find((n) => n.id === id);
  };

  it('edits when the step is done, and nothing else of it', async () => {
    const { onChange, node, workflow } = show();

    await userEvent.type(screen.getByLabelText(/Done when/), 'X');

    const next = changed(onChange, workflow, node.id);
    expect(next?.type === 'agent' && next.completionCriteria).toBe('X');
    expect(next?.type === 'agent' && next.instructions).toBe(node.instructions);
  });

  it('is not read-only until someone says so, and then only the file-editing tools are held back', async () => {
    const { onChange, node, workflow } = show();
    const box = screen.getByRole('checkbox', { name: /Read-only step/ });
    expect(box).not.toBeChecked();

    await userEvent.click(box);

    const next = changed(onChange, workflow, node.id);
    expect(next?.type === 'agent' && next.executionPolicy).toEqual({
      isolation: 'from_agent',
      announceIntegrations: true,
      readOnly: true,
    });
    expect(screen.getByText(/Tools granted below are not limited by this/)).toBeInTheDocument();
  });
});

describe('a handoff laid out as the design draws it', () => {
  it('shows who gave it to whom, the numbered sections and the files as rows with their counts', () => {
    const workflow = passwordRecovery();
    const result = run(workflow, 'running', { qa: nodeState('completed') });
    render(
      <I18nProvider language="en-US">
        <HandoffView
          run={result}
          index={1}
          agentName={(id) => `agent ${id}`}
          onOpenExecution={vi.fn()}
          handoff={handoff('qa', 'bug-fixer', {
            status: 'success',
            outcome: 'fixed',
            instructions: 'Run the recurrence tests.',
            changedFiles: [
              {
                path: 'src/main/Service.java',
                oldPath: null,
                status: 'modified',
                additions: 47,
                deletions: 2,
                binary: false,
              },
            ],
          })}
        />
      </I18nProvider>,
    );

    expect(screen.getByText('HANDOFF #1')).toBeInTheDocument();
    expect(screen.getByText('Sending agent')).toBeInTheDocument();
    expect(screen.getByText(/01 · Output contract & result/)).toBeInTheDocument();
    expect(screen.getByText(/02 · Shared state & context/)).toBeInTheDocument();
    expect(screen.getByText(/04 · Suggested next action/)).toBeInTheDocument();
    // The file is a row: its name, its folder and what changed in it.
    expect(screen.getByText('Service.java')).toBeInTheDocument();
    expect(screen.getByText('src/main')).toBeInTheDocument();
    expect(screen.getByText('+47')).toBeInTheDocument();
    expect(screen.getByText('−2')).toBeInTheDocument();
  });
});
