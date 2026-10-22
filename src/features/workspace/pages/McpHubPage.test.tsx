import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { I18nProvider } from '@/i18n/I18nProvider';
import type {
  McpConnectionDto,
  McpPresetDto,
  McpRegistryPageDto,
  McpSyncPlanDto,
  RuntimeStatusDto,
} from '@/lib/tauri/commands';
import { agent } from '@/test/fixtures';
import * as mcp from '../services/mcpService';
import { McpHubPage } from './McpHubPage';

vi.mock('../services/mcpService');
vi.mock('../hooks/WorkspaceProvider', () => ({
  useWorkspace: () => ({ active: { id: 'w1', name: 'Oficina' } }),
}));

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

const antigravity = {
  runtime: {
    id: 'antigravity',
    name: 'Antigravity CLI',
    capabilities: {
      mcp: 'cli_configured',
      mcpFeatures: { toolFilter: 'unsupported', probe: 'none', strict: false },
    },
  },
} as unknown as RuntimeStatusDto;

vi.mock('@/features/agents/hooks/useCatalog', () => ({
  useCatalog: () => ({
    catalog: {
      status: 'ready',
      personalities: [],
      agents: [agent('a1', 'Architect'), agent('qa', 'QA TESTER', 'antigravity')],
    },
    runtimes: { status: 'ready', runtimes: [claude, antigravity] },
  }),
}));

const devtools: McpPresetDto = {
  id: 'chrome-devtools',
  connectionName: 'chrome-devtools',
  availability: 'ready',
  risk: 'high',
  sourceUrl: 'https://example.test/devtools',
  pinnedVersion: '1.10.1',
  command: 'npx -y chrome-devtools-mcp@1.10.1 --isolated',
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

function show(connections: McpConnectionDto[] = []) {
  vi.mocked(mcp.listMcpCatalog).mockResolvedValue([devtools]);
  vi.mocked(mcp.listMcpConnections).mockResolvedValue({ connections, grants: [] });
  vi.mocked(mcp.listWorkspaceWorkflows).mockResolvedValue([]);
  return render(
    <I18nProvider language="en-US">
      <McpHubPage />
    </I18nProvider>,
  );
}

/** The first of what a query found; a test that finds nothing should say so, not crash on undefined. */
function first<T>(items: T[]): T {
  const [item] = items;
  if (item === undefined) throw new Error('nothing found');
  return item;
}

describe('McpHubPage', () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it('counts what the core reported and filters by what is typed', async () => {
    show([
      connection({
        enabled: true,
        discovery: { discoveredAt: 1, runtimeId: 'claude', status: 'connected', tools: ['a', 'b'] },
      }),
      connection({ id: 'c2', name: 'files', transport: { kind: 'stdio', executable: 'node' } }),
    ]);

    const list = await screen.findByRole('list', { name: 'MCP servers' });
    expect(within(list).getAllByRole('listitem')).toHaveLength(2);
    const summary = screen.getByRole('list', { name: 'Hub summary' });
    expect(within(summary).getByText('connected').closest('li')).toHaveTextContent('1');
    expect(within(summary).getByText('tools').closest('li')).toHaveTextContent('2');

    await userEvent.type(screen.getByLabelText(/Search server/), 'files');
    expect(within(list).getAllByRole('listitem')).toHaveLength(1);
    expect(screen.getByRole('listitem', { name: 'files' })).toBeInTheDocument();
  });

  it('says on the card that an agent on Antigravity gets the server through the CLI’s own configuration, and copies the command to add it', async () => {
    vi.mocked(mcp.listMcpCatalog).mockResolvedValue([devtools]);
    vi.mocked(mcp.listMcpConnections).mockResolvedValue({
      connections: [connection({ enabled: true })],
      grants: [
        {
          id: 'g1',
          connectionId: 'c1',
          agentId: 'qa',
          workflowId: null,
          nodeId: null,
          tools: { kind: 'server' },
        },
      ],
    });
    vi.mocked(mcp.listWorkspaceWorkflows).mockResolvedValue([]);
    vi.mocked(mcp.exportMcpConfig).mockResolvedValue('agy mcp add chrome-devtools -- npx\n');
    const write = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: write },
      configurable: true,
    });
    render(
      <I18nProvider language="en-US">
        <McpHubPage />
      </I18nProvider>,
    );

    const card = await screen.findByRole('listitem', { name: 'chrome-devtools' });
    // The warnings of a card are folded, with their count and tone showing.
    await userEvent.click(within(card).getByRole('button', { name: /warning/ }));
    expect(within(card).getByText(/QA TESTER runs on Antigravity CLI/)).toBeInTheDocument();
    await userEvent.click(within(card).getByRole('button', { name: 'Copy the command to add it' }));

    expect(mcp.exportMcpConfig).toHaveBeenCalledWith('w1', 'antigravity', ['c1']);
    await waitFor(() => {
      expect(write).toHaveBeenCalledWith('agy mcp add chrome-devtools -- npx\n');
    });
  });

  it('unticking an agent that has the server revokes it on Apply, and a narrower grant stays listed apart', async () => {
    vi.mocked(mcp.listMcpCatalog).mockResolvedValue([devtools]);
    vi.mocked(mcp.listMcpConnections).mockResolvedValue({
      connections: [connection({ enabled: true })],
      grants: [
        {
          id: 'g1',
          connectionId: 'c1',
          agentId: 'a1',
          workflowId: null,
          nodeId: null,
          tools: { kind: 'server' },
        },
        {
          id: 'g2',
          connectionId: 'c1',
          agentId: 'qa',
          workflowId: null,
          nodeId: null,
          tools: { kind: 'only', tools: ['x'] },
        },
      ],
    });
    vi.mocked(mcp.listWorkspaceWorkflows).mockResolvedValue([]);
    vi.mocked(mcp.revokeMcpGrant).mockResolvedValue(undefined);
    render(
      <I18nProvider language="en-US">
        <McpHubPage />
      </I18nProvider>,
    );

    const architect = await screen.findByRole('checkbox', { name: /Architect/ });
    expect(architect).toBeChecked();
    expect(screen.getByRole('checkbox', { name: /QA TESTER/ })).not.toBeChecked();
    expect(screen.getByRole('button', { name: 'Apply' })).toBeDisabled();
    expect(screen.getByText('Narrower grants')).toBeInTheDocument();

    await userEvent.click(architect);
    await userEvent.click(screen.getByRole('button', { name: 'Apply (1)' }));

    await waitFor(() => {
      expect(mcp.revokeMcpGrant).toHaveBeenCalledWith('g1');
    });
    expect(mcp.revokeMcpGrant).not.toHaveBeenCalledWith('g2');
  });

  it('remote servers are not offered: their filter is disabled and says why', async () => {
    show([connection()]);

    const remote = await screen.findByRole('button', { name: /Remote \(HTTP\/SSE\)/ });

    expect(remote).toBeDisabled();
    expect(remote).toHaveAttribute('title', expect.stringContaining('not supported yet'));
  });

  it('switching on says what it means first and needs a second click', async () => {
    show([connection()]);
    vi.mocked(mcp.setMcpConnectionEnabled).mockResolvedValue(connection({ enabled: true }));

    await userEvent.click(await screen.findByRole('switch', { name: /chrome-devtools/ }));
    expect(screen.getByText(/start this program/)).toBeInTheDocument();
    expect(mcp.setMcpConnectionEnabled).not.toHaveBeenCalled();

    const group = screen.getByRole('group', { name: 'Switch on' });
    await userEvent.click(first(within(group).getAllByRole('button')));

    await waitFor(() => {
      expect(mcp.setMcpConnectionEnabled).toHaveBeenCalledWith('c1', true);
    });
  });

  it('a secret is handed over and the field emptied; granting needs its own click', async () => {
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
    await screen.findByRole('complementary', { name: 'MCP inspector' });

    // Ticking changes nothing by itself; Apply does it, for everything ticked at once.
    await userEvent.click(screen.getByRole('checkbox', { name: /Architect/ }));
    await userEvent.click(screen.getByRole('checkbox', { name: /QA TESTER/ }));
    expect(mcp.grantMcpConnection).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: 'Apply (2)' }));
    await waitFor(() => {
      expect(mcp.grantMcpConnection).toHaveBeenCalledTimes(2);
    });
    expect(mcp.grantMcpConnection).toHaveBeenCalledWith('c1', 'a1', { kind: 'server' }, {});
    expect(mcp.grantMcpConnection).toHaveBeenCalledWith('c1', 'qa', { kind: 'server' }, {});

    await userEvent.click(screen.getByRole('tab', { name: 'Variables & env' }));
    const input = screen.getByLabelText(/Value of API_TOKEN/);
    await userEvent.type(input, 'hunter2');
    await userEvent.click(screen.getByRole('button', { name: 'Store' }));
    expect(mcp.setMcpSecret).toHaveBeenCalledWith('c1', 'API_TOKEN', 'hunter2');
    expect(input).toHaveValue('');
  });

  it('the manifest names a secret and never shows a value', async () => {
    show([
      connection({
        secrets: { API_TOKEN: 1 },
        transport: {
          kind: 'stdio',
          executable: 'node',
          env: [{ name: 'API_TOKEN', value: { kind: 'secret' } }],
        },
      }),
    ]);
    await screen.findByRole('complementary', { name: 'MCP inspector' });

    await userEvent.click(screen.getByRole('tab', { name: 'JSON manifest' }));

    expect(screen.getByText(/API_TOKEN/)).toBeInTheDocument();
    expect(screen.getByText(/<secret: stored in the system credential store>/)).toBeInTheDocument();
  });

  it('the registry shows how to connect, asks for what is required and adds a switched-off server', async () => {
    show([]);
    const page: McpRegistryPageDto = {
      nextCursor: null,
      servers: [
        {
          name: 'io.github.acme/jira',
          title: 'Jira',
          description: 'Issues',
          version: '1.2.0',
          repositoryUrl: 'https://github.com/acme/jira',
          websiteUrl: null,
          installs: [
            {
              kind: 'npm',
              executable: 'npx',
              args: ['-y', '@acme/jira-mcp@1.2.0'],
              inputs: [{ flag: '--site', value: null, description: 'Your site', required: true }],
              env: [
                {
                  name: 'JIRA_TOKEN',
                  description: null,
                  required: true,
                  secret: true,
                  default: null,
                },
              ],
            },
          ],
          remotes: [],
          unsupportedPackages: [],
        },
        {
          name: 'ai.x/hosted',
          title: null,
          description: 'Hosted',
          version: '1',
          repositoryUrl: null,
          websiteUrl: null,
          installs: [],
          remotes: ['https://x/mcp'],
          unsupportedPackages: [],
        },
      ],
    };
    vi.mocked(mcp.searchMcpRegistry).mockResolvedValue(page);
    vi.mocked(mcp.addMcpConnection).mockResolvedValue(connection({ id: 'new', name: 'jira' }));
    vi.mocked(mcp.setMcpSecret).mockResolvedValue(connection());

    await userEvent.click(first(await screen.findAllByRole('button', { name: 'Add MCP server' })));
    await userEvent.click(screen.getByRole('tab', { name: 'MCP Registry' }));
    await userEvent.type(screen.getByLabelText('Search servers (Jira, GitHub, Postgres…)'), 'jira');
    await userEvent.click(screen.getByRole('button', { name: 'Search' }));

    expect(mcp.searchMcpRegistry).toHaveBeenCalledWith('jira', undefined);
    const hosted = await screen.findByRole('listitem', { name: 'ai.x/hosted' });
    expect(within(hosted).getByText('remote only')).toBeInTheDocument();

    const row = screen.getByRole('listitem', { name: 'io.github.acme/jira' });
    await userEvent.click(within(row).getByRole('button', { name: /Jira/ }));
    expect(within(row).getByText(/^npx -y @acme\/jira-mcp@1\.2\.0/)).toBeInTheDocument();
    const add = within(row).getByRole('button', { name: /Add/ });
    // The required site and the required secret are asked for before anything is added.
    expect(add).toBeDisabled();
    await userEvent.type(within(row).getByLabelText(/--site/), 'acme.atlassian.net');
    await userEvent.type(within(row).getByLabelText(/JIRA_TOKEN/), 'tok');
    // A secret does not stop the add: it is stored after.
    await waitFor(() => {
      expect(add).toBeEnabled();
    });
    await userEvent.click(add);

    await waitFor(() => {
      expect(mcp.addMcpConnection).toHaveBeenCalledWith(
        'w1',
        'jira',
        {
          kind: 'stdio',
          executable: 'npx',
          args: ['-y', '@acme/jira-mcp@1.2.0', '--site', 'acme.atlassian.net'],
          env: [{ name: 'JIRA_TOKEN', value: { kind: 'secret' } }],
        },
        false,
      );
    });
    expect(mcp.setMcpSecret).toHaveBeenCalledWith('new', 'JIRA_TOKEN', 'tok');
    expect(mcp.setMcpConnectionEnabled).not.toHaveBeenCalled();
    expect(mcp.grantMcpConnection).not.toHaveBeenCalled();
  });

  it('a sync shows what each CLI lacks and one button brings them in step', async () => {
    show([connection()]);
    const plan: McpSyncPlanDto = {
      runtimes: [
        { dialect: 'claude', path: '/p/.mcp.json', available: true, error: null },
        { dialect: 'gemini', path: '/h/.gemini/settings.json', available: true, error: null },
        { dialect: 'open_code', path: null, available: false, error: null },
        { dialect: 'codex', path: '/h/.codex/config.toml', available: true, error: 'unreadable' },
      ],
      servers: [
        {
          name: 'jira',
          inAtlas: false,
          foundIn: ['gemini'],
          missingFrom: ['claude'],
        },
      ],
      skipped: [{ dialect: 'gemini', name: 'web', reason: 'http_not_supported' }],
    };
    vi.mocked(mcp.planMcpSync).mockResolvedValue(plan);
    vi.mocked(mcp.applyMcpSync).mockResolvedValue({
      imported: [
        {
          connectionId: 'x',
          name: 'jira',
          secretsStored: [],
          secretsMissing: ['JIRA_TOKEN'],
          credentialStoreFailed: false,
          warnings: [],
        },
      ],
      applied: [
        { dialect: 'claude', path: '/p/.mcp.json', added: ['jira'], backup: null, error: null },
      ],
      skipped: [],
    });

    await userEvent.click(await screen.findByRole('button', { name: 'Configuration' }));
    await userEvent.click(screen.getByRole('menuitem', { name: 'Sync CLIs' }));

    expect(await screen.findByLabelText('jira: Claude Code: will be added')).toBeInTheDocument();
    expect(screen.getByLabelText('jira: Gemini CLI: has it')).toBeInTheDocument();
    expect(screen.getByLabelText('jira: Atlas: will be brought into Atlas')).toBeInTheDocument();
    expect(screen.getByText('not readable (left alone)')).toBeInTheDocument();
    expect(screen.getByText(/1 server\(s\) cannot be synced/)).toBeInTheDocument();
    expect(mcp.applyMcpSync).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole('button', { name: /^Sync: add 1 to 1 file/ }));

    expect(mcp.applyMcpSync).toHaveBeenCalledWith('w1');
    expect(
      await screen.findByText(/1 brought into Atlas, 1 added to CLI files/),
    ).toBeInTheDocument();
    expect(screen.getByText(/jira:JIRA_TOKEN/)).toBeInTheDocument();
  });

  it('importing a configuration reports what came in and what was left out, and why', async () => {
    show([connection()]);
    vi.mocked(mcp.importMcpConfig).mockResolvedValue({
      dialect: 'mcp',
      added: [
        {
          connectionId: 'n1',
          name: 'api',
          secretsStored: ['API_KEY'],
          secretsMissing: [],
          credentialStoreFailed: false,
          warnings: ['tool_filter_not_imported'],
        },
      ],
      skipped: [{ name: 'figma', reason: 'http_not_supported' }],
    });

    await userEvent.click(await screen.findByRole('button', { name: 'Configuration' }));
    await userEvent.click(screen.getByRole('menuitem', { name: /atlas-mcp.json/ }));
    await userEvent.type(within(screen.getByRole('dialog')).getByLabelText('Configuration'), 'x');
    await userEvent.click(screen.getByRole('button', { name: /^Import/ }));

    expect(mcp.importMcpConfig).toHaveBeenCalledWith('w1', 'x', undefined);
    expect(await screen.findByText(/credential stored: API_KEY/)).toBeInTheDocument();
    expect(screen.getByText(/tool filter was not copied/)).toBeInTheDocument();
    expect(
      screen.getByText(/remote \(HTTP\/SSE\) servers are not supported yet/),
    ).toBeInTheDocument();
  });
});
