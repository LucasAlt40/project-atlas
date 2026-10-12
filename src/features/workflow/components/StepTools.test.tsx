import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { I18nProvider } from '@/i18n/I18nProvider';
import type { McpConnectionDto, McpGrantDto } from '@/lib/tauri/commands';
import * as mcp from '@/features/workspace/services/mcpService';
import { firstAgentStep, passwordRecovery } from '@/test/workflowFixtures';
import type { AgentNode } from '../types';
import { StepTools } from './StepTools';

vi.mock('@/features/workspace/services/mcpService');

const connection = (over: Partial<McpConnectionDto>): McpConnectionDto => ({
  id: 'c1',
  workspaceId: 'w1',
  name: 'jira',
  transport: { kind: 'stdio', executable: 'npx' },
  enabled: true,
  required: false,
  secrets: {},
  createdAt: 1,
  ...over,
});

function show(
  connections: McpConnectionDto[],
  grants: McpGrantDto[] = [],
  { saved = true, agentId = '', readOnly = false } = {},
) {
  const workflow = passwordRecovery();
  const found = firstAgentStep(workflow);
  const node: AgentNode = {
    ...found,
    ...(agentId ? { agentId } : {}),
    executionPolicy: { ...found.executionPolicy, readOnly },
  };
  vi.mocked(mcp.listMcpConnections).mockResolvedValue({ connections, grants });
  vi.mocked(mcp.grantMcpConnection).mockResolvedValue({
    id: 'g-new',
    connectionId: 'c1',
    agentId: node.agentId,
    workflowId: workflow.id,
    nodeId: node.id,
    tools: { kind: 'server' },
  });
  vi.mocked(mcp.revokeMcpGrant).mockResolvedValue(undefined);
  render(
    <I18nProvider language="en-US">
      <StepTools workspaceId="w1" workflow={workflow} node={node} saved={saved} />
    </I18nProvider>,
  );
  return { workflow, node };
}

describe('the tools of a step', () => {
  it('grants a connection to this step and this agent only, whole or limited to what reads', async () => {
    const user = userEvent.setup();
    const { workflow, node } = show([
      connection({
        discovery: {
          discoveredAt: 1,
          runtimeId: 'claude',
          status: 'connected',
          tools: ['getJiraIssue', 'createJiraIssue'],
        },
      }),
    ]);
    await user.selectOptions(await screen.findByLabelText('Connection'), 'c1');

    await user.selectOptions(screen.getByLabelText('Tools'), 'reading');
    await user.click(screen.getByRole('button', { name: 'Grant to this step' }));

    expect(mcp.grantMcpConnection).toHaveBeenCalledWith(
      'c1',
      node.agentId,
      { kind: 'only', tools: ['getJiraIssue'] },
      { workflowId: workflow.id, nodeId: node.id },
    );
  });

  it('offers the reading-only choice only when the tools of the server are known', async () => {
    const user = userEvent.setup();
    show([connection({})]);
    await user.selectOptions(await screen.findByLabelText('Connection'), 'c1');

    const option = within(screen.getByLabelText('Tools')).getByRole('option', {
      name: /Only the ones that read/,
    });
    expect(option).toBeDisabled();
  });

  it("does not show a grant of the whole workflow or of another step as this step's", async () => {
    const workflow = passwordRecovery();
    show(
      [connection({}), connection({ id: 'c2', name: 'files' })],
      [
        {
          id: 'g1',
          connectionId: 'c1',
          agentId: null,
          workflowId: workflow.id,
          nodeId: null,
          tools: { kind: 'server' },
        },
        {
          id: 'g2',
          connectionId: 'c2',
          agentId: null,
          workflowId: workflow.id,
          nodeId: 'another-step',
          tools: { kind: 'server' },
        },
      ],
    );

    expect(await screen.findByText('No tool is granted to this step.')).toBeInTheDocument();
  });

  it('shows and revokes a grant made to this step', async () => {
    const user = userEvent.setup();
    const workflow = passwordRecovery();
    const found = firstAgentStep(workflow);
    show(
      [connection({})],
      [
        {
          id: 'g1',
          connectionId: 'c1',
          agentId: found.agentId,
          workflowId: workflow.id,
          nodeId: found.id,
          tools: { kind: 'only', tools: ['getJiraIssue'] },
        },
      ],
    );

    expect(await screen.findByText('only: getJiraIssue')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Revoke jira' }));

    await waitFor(() => {
      expect(mcp.revokeMcpGrant).toHaveBeenCalledWith('g1');
    });
  });

  it('says what is missing instead of offering a grant that cannot be made', async () => {
    show([], [], { saved: false });
    expect(
      await screen.findByText('Save the workflow before granting tools to a new step.'),
    ).toBeInTheDocument();
  });

  it('says so when no connection is switched on', async () => {
    show([connection({ enabled: false })]);
    expect(await screen.findByText(/no enabled connection/)).toBeInTheDocument();
  });

  it('reminds that a read-only step still acts through the tools it is granted', async () => {
    const workflow = passwordRecovery();
    const found = firstAgentStep(workflow);
    show(
      [connection({})],
      [
        {
          id: 'g1',
          connectionId: 'c1',
          agentId: found.agentId,
          workflowId: workflow.id,
          nodeId: found.id,
          tools: { kind: 'server' },
        },
      ],
      { readOnly: true },
    );

    expect(await screen.findByText(/can still act on what it connects to/)).toBeInTheDocument();
  });
});
