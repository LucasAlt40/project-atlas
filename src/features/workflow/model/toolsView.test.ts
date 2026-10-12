import type { McpConnectionDto, McpGrantDto } from '@/lib/tauri/commands';
import { passwordRecovery } from '@/test/workflowFixtures';
import { connectionViews } from './toolsView';

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
const grant = (over: Partial<McpGrantDto>): McpGrantDto => ({
  id: 'g',
  connectionId: 'c-jira',
  agentId: null,
  workflowId: null,
  nodeId: null,
  tools: { kind: 'server' },
  ...over,
});

describe('the connections as one workflow sees them', () => {
  const workflow = passwordRecovery();

  it('names the steps of this workflow a connection is granted to, and only those', () => {
    const [view] = connectionViews(
      workflow,
      [connection('jira')],
      [
        grant({
          id: 'g1',
          workflowId: workflow.id,
          nodeId: 'qa',
          tools: { kind: 'only', tools: ['get'] },
        }),
        grant({ id: 'g2', workflowId: 'another-workflow', nodeId: 'qa' }),
      ],
    );

    expect(view?.steps).toEqual([
      { nodeId: 'qa', label: 'QA', tools: { kind: 'only', tools: ['get'] } },
    ]);
    expect(view?.broad).toBe(0);
  });

  it('counts a grant to the whole workflow, or to an agent anywhere, apart from the steps', () => {
    const [view] = connectionViews(
      workflow,
      [connection('jira')],
      [grant({ id: 'g1', workflowId: workflow.id }), grant({ id: 'g2', agentId: 'a-qa' })],
    );

    expect(view?.steps).toEqual([]);
    expect(view?.broad).toBe(2);
  });

  it('reports the last look at the server as it is, and none when it was never looked at', () => {
    const views = connectionViews(
      workflow,
      [
        connection('seen', {
          discovery: { discoveredAt: 1, runtimeId: 'claude', status: 'failed', tools: [] },
        }),
        connection('unseen'),
      ],
      [],
    );

    expect(views.map((v) => v.status)).toEqual(['failed', null]);
  });
});
