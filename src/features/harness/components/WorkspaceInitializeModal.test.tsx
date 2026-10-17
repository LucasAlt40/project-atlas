import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {
  agent,
  mockBackend,
  projectAnalysis,
  renderWithProviders,
  workspace,
} from '@/test/fixtures';
import { analyzeProject } from '@/features/workspace/services/workspaceService';
import type { SemanticStatusDto } from '@/lib/tauri/commands';
import { WorkspaceInitializeModal } from './WorkspaceInitializeModal';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');

const projects = [
  { id: 'p1', name: 'backend', path: '/dev/erp/backend' },
  { id: 'p2', name: 'frontend', path: '/dev/erp/frontend' },
];

const withAi = (status: SemanticStatusDto) => {
  const base = projectAnalysis();
  return projectAnalysis({
    analysis: {
      ...base.analysis,
      semantic: {
        status,
        error: status === 'failed' ? 'semantic_analysis_failed' : null,
        errorDetail: null,
        sentFiles: [],
        rejected: 0,
        explored: true,
      },
    },
  });
};

describe('WorkspaceInitializeModal', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockBackend({
      workspaces: [workspace('w1', 'ERP', '/dev/erp/backend')],
      agents: [agent('a1', 'Analyst')],
    });
  });

  it('says which project the AI did not analyse, on every tab, and reruns only that one', async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <WorkspaceInitializeModal
        workspaceId="w1"
        projects={projects}
        onClose={vi.fn()}
        onDone={vi.fn()}
      />,
    );
    await screen.findByRole('button', { name: /Analyse all with AI/ });
    let frontendRuns = 0;
    vi.mocked(analyzeProject).mockImplementation((_w, request, projectId) => {
      if (!request) return Promise.resolve(projectAnalysis());
      if (projectId === 'p1') return Promise.resolve(withAi('completed'));
      frontendRuns += 1;
      return Promise.resolve(withAi(frontendRuns === 1 ? 'failed' : 'completed'));
    });

    await user.click(screen.getByRole('button', { name: /Analyse all with AI/ }));

    // The backend tab is the active one and it worked, yet the failure of the other is told.
    const alert = await screen.findByRole('alert', { name: '' });
    expect(alert).toHaveTextContent('The AI did not analyse: frontend.');
    expect(within(screen.getByRole('tab', { name: /frontend/ })).getByText('!')).toBeVisible();

    await user.click(screen.getByRole('button', { name: /Analyse all with AI/ }));

    await waitFor(() => {
      expect(screen.queryByText(/The AI did not analyse/)).not.toBeInTheDocument();
    });
    const runs = vi.mocked(analyzeProject).mock.calls.filter(([, request]) => request);
    // Two on the first round, then only the frontend: the backend already had its analysis.
    expect(runs.map(([, , id]) => id)).toEqual(['p1', 'p2', 'p2']);
  });
});
