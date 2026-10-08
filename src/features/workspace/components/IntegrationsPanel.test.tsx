import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { I18nProvider } from '@/i18n/I18nProvider';
import type { McpConnectionDto, McpPresetDto, RuntimeStatusDto } from '@/lib/tauri/commands';
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
  runtime: { id: 'claude', name: 'Claude Code', capabilities: { mcp: 'supported' } },
} as unknown as RuntimeStatusDto;

function show(connections: McpConnectionDto[] = []) {
  vi.mocked(mcp.listMcpCatalog).mockResolvedValue([devtools, figma]);
  vi.mocked(mcp.listMcpConnections).mockResolvedValue({ connections, grants: [] });
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
    expect(mcp.grantMcpConnection).toHaveBeenCalledWith('c1', 'a1', { kind: 'server' });
  });
});
