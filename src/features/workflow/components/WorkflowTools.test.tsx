import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { I18nProvider } from '@/i18n/I18nProvider';
import type { McpConnectionDto } from '@/lib/tauri/commands';
import * as mcp from '@/features/workspace/services/mcpService';
import { passwordRecovery } from '@/test/workflowFixtures';
import { WorkflowTools } from './WorkflowTools';

vi.mock('@/features/workspace/services/mcpService');

const connection = (name: string, extra: Partial<McpConnectionDto> = {}): McpConnectionDto => ({
  id: `c-${name}`,
  workspaceId: 'w1',
  name,
  transport: { kind: 'stdio', executable: 'npx' },
  enabled: true,
  required: false,
  secrets: {},
  createdAt: 1,
  ...extra,
});

function show(onOpenHub = vi.fn()) {
  render(
    <I18nProvider language="en-US">
      <WorkflowTools workspaceId="w1" workflow={passwordRecovery()} onOpenHub={onOpenHub} />
    </I18nProvider>,
  );
  return onOpenHub;
}

describe('the tools card of a workflow', () => {
  it('shows each connection in the state it really is in, and where it is granted', async () => {
    const workflow = passwordRecovery();
    vi.mocked(mcp.listMcpConnections).mockResolvedValue({
      connections: [
        connection('chrome-devtools', {
          discovery: {
            discoveredAt: 1,
            runtimeId: 'claude',
            status: 'connected',
            tools: ['list_pages'],
          },
        }),
        connection('jira', { enabled: false }),
      ],
      grants: [
        {
          id: 'g1',
          connectionId: 'c-chrome-devtools',
          agentId: null,
          workflowId: workflow.id,
          nodeId: 'qa',
          tools: { kind: 'server' },
        },
      ],
    });

    show();

    expect(await screen.findByText('chrome-devtools')).toBeInTheDocument();
    expect(screen.getByText('on · connected')).toBeInTheDocument();
    expect(screen.getByText('off · never checked')).toBeInTheDocument();
    expect(screen.getByText('QA')).toBeInTheDocument();
    expect(screen.getByText('not granted to any step of this workflow')).toBeInTheDocument();
  });

  it('says when the workspace has no connection, and when they cannot be read', async () => {
    vi.mocked(mcp.listMcpConnections).mockResolvedValue({ connections: [], grants: [] });
    show();
    expect(
      await screen.findByText('This workspace has no MCP connection yet.'),
    ).toBeInTheDocument();
  });

  it('does not invent a state when the connections cannot be read', async () => {
    vi.mocked(mcp.listMcpConnections).mockRejectedValue(new Error('no'));
    show();
    expect(await screen.findByText('The connections could not be read.')).toBeInTheDocument();
  });

  it('leads to the MCP hub, where connections are added and switched on', async () => {
    vi.mocked(mcp.listMcpConnections).mockResolvedValue({ connections: [], grants: [] });
    const onOpenHub = show();

    await userEvent.click(await screen.findByRole('button', { name: 'Manage in the MCP hub' }));

    expect(onOpenHub).toHaveBeenCalled();
  });
});
