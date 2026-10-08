import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { I18nProvider } from '@/i18n/I18nProvider';
import type {
  McpConnectionDto,
  McpPresetDto,
  RuntimeStatusDto,
  WorkflowDto,
} from '@/lib/tauri/commands';
import { agent } from '@/test/fixtures';
import * as mcp from '../services/mcpService';
import { IntegrationsPanel } from './IntegrationsPanel';

vi.mock('../services/mcpService');

const devtools: McpPresetDto = {
  id: 'chrome-devtools',
  connectionName: 'chrome-devtools',
  availability: 'ready',
  risk: 'high',
  sourceUrl: 'https://example.test/devtools',
  pinnedVersion: '1.10.1',
  command: 'npx -y chrome-devtools-mcp@1.10.1 --isolated',
  requirements: [
    { requirement: 'node', found: true },
    { requirement: 'npx', found: true },
    { requirement: 'chrome', found: false },
  ],
};
const figma: McpPresetDto = {
  id: 'figma',
  connectionName: 'figma',
  availability: 'needs_http_and_oauth',
  risk: 'high',
  sourceUrl: 'https://example.test/figma',
  pinnedVersion: null,
  command: null,
  requirements: [],
};
const connection = (over: Partial<McpConnectionDto> = {}): McpConnectionDto => ({
  id: 'c1',
  workspaceId: 'w1',
  name: 'chrome-devtools',
  transport: { kind: 'stdio', executable: 'npx', args: ['-y', 'chrome-devtools-mcp@1.10.1'] },
  enabled: false,
  required: false,
  secrets: {},
  createdAt: 1,
  ...over,
});
const claude = {
  runtime: {
    id: 'claude',
    name: 'Claude Code',
    capabilities: {
      mcp: 'supported',
      mcpFeatures: { toolFilter: 'deny_list', probe: 'tools', strict: true },
    },
  },
} as unknown as RuntimeStatusDto;

function show(connections: McpConnectionDto[] = [], workflows: WorkflowDto[] = []) {
  vi.mocked(mcp.listMcpCatalog).mockResolvedValue([devtools, figma]);
  vi.mocked(mcp.listMcpConnections).mockResolvedValue({ connections, grants: [] });
  vi.mocked(mcp.listWorkspaceWorkflows).mockResolvedValue(workflows);
  return render(
    <I18nProvider language="en-US">
      <IntegrationsPanel
        workspaceId="w1"
        agents={[agent('a1', 'Architect')]}
        runtimes={[claude]}
        onClose={() => undefined}
      />
    </I18nProvider>,
  );
}

describe('IntegrationsPanel', () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it('lists what Atlas knows, pinned, with Figma shown for what it is and not addable', async () => {
    show();

    const card = await screen.findByRole('listitem', { name: 'chrome-devtools' });
    expect(within(card).getByText(/Pinned to version 1\.10\.1/)).toBeInTheDocument();
    expect(within(card).getByRole('button', { name: /Add/ })).toBeEnabled();
    const planned = screen.getByRole('listitem', { name: 'figma' });
    expect(within(planned).getByRole('button', { name: /Add/ })).toBeDisabled();
    expect(within(planned).getByText(/OAuth/)).toBeInTheDocument();
  });

  it('adding creates a connection and does not switch anything on', async () => {
    show();
    vi.mocked(mcp.addMcpPreset).mockResolvedValue(connection());

    await userEvent.click(
      within(await screen.findByRole('listitem', { name: 'chrome-devtools' })).getByRole('button', {
        name: /Add/,
      }),
    );

    expect(mcp.addMcpPreset).toHaveBeenCalledWith('w1', 'chrome-devtools');
    expect(mcp.setMcpConnectionEnabled).not.toHaveBeenCalled();
    expect(mcp.grantMcpConnection).not.toHaveBeenCalled();
  });

  it('switching on says what it means first and needs a second click', async () => {
    show([connection()]);
    vi.mocked(mcp.setMcpConnectionEnabled).mockResolvedValue(connection({ enabled: true }));
    const card = (await screen.findAllByRole('listitem', { name: 'chrome-devtools' })).at(-1);
    if (!card) throw new Error('no card');

    await userEvent.click(within(card).getByRole('button', { name: /Switch on/ }));
    expect(within(card).getByText(/start this program/)).toBeInTheDocument();
    expect(within(card).getByText(/high risk|exposes the content/i)).toBeInTheDocument();
    expect(mcp.setMcpConnectionEnabled).not.toHaveBeenCalled();

    await userEvent.click(
      within(card).getByRole('group', { name: 'Switch on' }).querySelector('button') as HTMLElement,
    );
    await waitFor(() => {
      expect(mcp.setMcpConnectionEnabled).toHaveBeenCalledWith('c1', true);
    });
  });

  it('a secret is handed over and the field is emptied; examining and granting need their own clicks', async () => {
    const withSecret = connection({
      enabled: true,
      transport: {
        kind: 'stdio',
        executable: 'node',
        args: [],
        env: [{ name: 'API_TOKEN', value: { kind: 'secret' } }],
      },
      discovery: { discoveredAt: 1, runtimeId: 'claude', status: 'connected', tools: ['read'] },
    });
    show([withSecret]);
    vi.mocked(mcp.setMcpSecret).mockResolvedValue(withSecret);
    vi.mocked(mcp.grantMcpConnection).mockResolvedValue({
      id: 'g1',
      connectionId: 'c1',
      agentId: 'a1',
      workflowId: null,
      nodeId: null,
      tools: { kind: 'server' },
    });
    const card = (await screen.findAllByRole('listitem', { name: 'chrome-devtools' })).at(-1);
    if (!card) throw new Error('no card');

    const input = within(card).getByLabelText(/Value of API_TOKEN/);
    await userEvent.type(input, 'hunter2');
    await userEvent.click(within(card).getByRole('button', { name: 'Store' }));
    expect(mcp.setMcpSecret).toHaveBeenCalledWith('c1', 'API_TOKEN', 'hunter2');
    expect(input).toHaveValue('');

    expect(mcp.probeMcpConnection).not.toHaveBeenCalled();
    await userEvent.click(within(card).getByRole('button', { name: 'Grant' }));
    expect(mcp.grantMcpConnection).toHaveBeenCalledWith('c1', 'a1', { kind: 'server' }, {});
  });

  it('a grant can be for one step of one workflow, and then it is for the agent that step runs', async () => {
    const flow = {
      id: 'wf1',
      name: 'Build',
      nodes: [
        { id: 'n1', type: 'agent', agentId: 'a1', label: 'Design' },
        { id: 'end', type: 'end', label: 'Done', outcome: 'done' },
      ],
    } as unknown as WorkflowDto;
    show([connection({ enabled: true })], [flow]);
    vi.mocked(mcp.grantMcpConnection).mockResolvedValue({
      id: 'g1',
      connectionId: 'c1',
      agentId: 'a1',
      workflowId: 'wf1',
      nodeId: 'n1',
      tools: { kind: 'server' },
    });
    const card = (await screen.findAllByRole('listitem', { name: 'chrome-devtools' })).at(-1);
    if (!card) throw new Error('no card');

    await userEvent.selectOptions(within(card).getByLabelText('Applies to'), 'wf1');
    // Only a step that runs an agent is offered.
    expect(within(card).queryByRole('option', { name: 'Done' })).toBeNull();
    await userEvent.selectOptions(within(card).getByLabelText('Step'), 'n1');
    await userEvent.click(within(card).getByRole('button', { name: 'Grant' }));

    expect(mcp.grantMcpConnection).toHaveBeenCalledWith(
      'c1',
      'a1',
      { kind: 'server' },
      { workflowId: 'wf1', nodeId: 'n1' },
    );
  });

  it('says so when the agent being granted runs on a runtime that cannot receive integrations', async () => {
    vi.mocked(mcp.listMcpCatalog).mockResolvedValue([devtools]);
    vi.mocked(mcp.listMcpConnections).mockResolvedValue({
      connections: [connection({ enabled: true })],
      grants: [],
    });
    vi.mocked(mcp.listWorkspaceWorkflows).mockResolvedValue([]);
    render(
      <I18nProvider language="en-US">
        <IntegrationsPanel
          workspaceId="w1"
          agents={[agent('qa', 'QA TESTER', 'antigravity')]}
          runtimes={[claude]}
          onClose={() => undefined}
        />
      </I18nProvider>,
    );

    const alerts = await screen.findAllByRole('alert');

    expect(alerts.some((a) => a.textContent.includes('Antigravity cannot'))).toBe(true);
  });

  it('an allow-list runtime takes tool names typed by hand, with no examination', async () => {
    const codex = {
      runtime: {
        id: 'codex',
        name: 'Codex',
        capabilities: {
          mcp: 'supported',
          mcpFeatures: { toolFilter: 'allow_list', probe: 'none', strict: false },
        },
      },
    } as unknown as RuntimeStatusDto;
    vi.mocked(mcp.listMcpCatalog).mockResolvedValue([devtools]);
    vi.mocked(mcp.listMcpConnections).mockResolvedValue({
      connections: [connection({ enabled: true })],
      grants: [],
    });
    vi.mocked(mcp.listWorkspaceWorkflows).mockResolvedValue([]);
    vi.mocked(mcp.grantMcpConnection).mockResolvedValue({
      id: 'g1',
      connectionId: 'c1',
      agentId: 'a1',
      workflowId: null,
      nodeId: null,
      tools: { kind: 'only', tools: ['navigate_page', 'take_screenshot'] },
    });
    render(
      <I18nProvider language="en-US">
        <IntegrationsPanel
          workspaceId="w1"
          agents={[agent('a1', 'Dev', 'codex')]}
          runtimes={[codex]}
          onClose={() => undefined}
        />
      </I18nProvider>,
    );
    const card = (await screen.findAllByRole('listitem', { name: 'chrome-devtools' })).at(-1);
    if (!card) throw new Error('no card');

    // It also loads what the user set up in it, and cannot be examined.
    expect(within(card).getByText(/loads the integrations set up in its own/)).toBeInTheDocument();
    expect(within(card).getByText(/No installed runtime can look at a server/)).toBeInTheDocument();
    await userEvent.click(within(card).getByLabelText('Only the tools I pick'));
    await userEvent.type(
      within(card).getByLabelText(/Tool names/),
      'navigate_page, take_screenshot',
    );
    await userEvent.click(within(card).getByRole('button', { name: 'Grant' }));

    expect(mcp.grantMcpConnection).toHaveBeenCalledWith(
      'c1',
      'a1',
      { kind: 'only', tools: ['navigate_page', 'take_screenshot'] },
      {},
    );
  });

  it('a runtime that cannot hold a server to named tools can only be granted the whole server', async () => {
    const opencode = {
      runtime: {
        id: 'opencode',
        name: 'OpenCode',
        capabilities: {
          mcp: 'supported',
          mcpFeatures: { toolFilter: 'unsupported', probe: 'status_only', strict: false },
        },
      },
    } as unknown as RuntimeStatusDto;
    vi.mocked(mcp.listMcpCatalog).mockResolvedValue([devtools]);
    vi.mocked(mcp.listMcpConnections).mockResolvedValue({
      connections: [
        connection({
          enabled: true,
          discovery: { discoveredAt: 1, runtimeId: 'opencode', status: 'connected', tools: [] },
        }),
      ],
      grants: [],
    });
    vi.mocked(mcp.listWorkspaceWorkflows).mockResolvedValue([]);
    render(
      <I18nProvider language="en-US">
        <IntegrationsPanel
          workspaceId="w1"
          agents={[agent('a1', 'Dev', 'opencode')]}
          runtimes={[opencode]}
          onClose={() => undefined}
        />
      </I18nProvider>,
    );
    const card = (await screen.findAllByRole('listitem', { name: 'chrome-devtools' })).at(-1);
    if (!card) throw new Error('no card');

    expect(within(card).getByLabelText('Only the tools I pick')).toBeDisabled();
    expect(within(card).getByText(/only "all its tools" can be granted/)).toBeInTheDocument();
  });
});
