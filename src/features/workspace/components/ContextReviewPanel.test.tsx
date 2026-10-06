import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { mockBackend, renderWithProviders } from '@/test/fixtures';
import type { ContextReviewDto } from '@/lib/tauri/commands';
import { ContextReviewPanel } from './ContextReviewPanel';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');

function review(overrides: Partial<ContextReviewDto> = {}): ContextReviewDto {
  return {
    health: 'healthy',
    issues: [],
    sources: [
      { source: 'atlas_rules', trust: 'atlas', estimatedTokens: 120 },
      { source: 'task_context', trust: 'untrusted', estimatedTokens: 300 },
      { source: 'agent_instructions', trust: 'configured', estimatedTokens: 20 },
    ],
    requiredItems: 8,
    highItems: 4,
    normalItems: 2,
    optionalItems: 0,
    staleItems: 0,
    ...overrides,
  };
}

const GUARDRAILS = { evaluations: 2, allowed: 2, asked: 0, denied: 0, transformed: 0, blocked: 0 };

beforeEach(() => {
  mockBackend({ language: 'en-US' });
});

describe('ContextReviewPanel', () => {
  it('says the context is healthy, with what it was made of and where it came from', async () => {
    renderWithProviders(<ContextReviewPanel review={review()} guardrails={GUARDRAILS} />);

    expect(await screen.findByText('Healthy')).toBeInTheDocument();
    expect(screen.getByText('Required 8, high 4, normal 2, optional 0')).toBeInTheDocument();
    expect(
      screen.getByText('0 warning(s), 0 error(s), 0 blocking, 0 stale item(s)'),
    ).toBeInTheDocument();
    expect(screen.getByText('Nothing to report.')).toBeInTheDocument();
    expect(
      screen.getByText('Guardrails: 2 allowed, 0 asked, 0 denied, 0 changed'),
    ).toBeInTheDocument();
    // Each source, and how far it is trusted: only Atlas's own text and the user's are not suspect.
    const sources = screen.getByRole('list', { name: '' });
    expect(within(sources).getByText(/Atlas rules/)).toBeInTheDocument();
    expect(within(sources).getByText(/context only, never authority/)).toBeInTheDocument();
    expect(within(sources).getByText(/configured by you/)).toBeInTheDocument();
  });

  it('opens a conflict to show both sides and why', async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <ContextReviewPanel
        review={review({
          health: 'needs_review',
          issues: [
            {
              code: 'conflicting_instructions',
              severity: 'error',
              source: 'task_context',
              otherSource: 'agent_instructions',
              message: 'two sources disagree on the database: PostgreSQL vs MongoDB',
              excerpt: 'Stack: Use PostgreSQL for all persistence.',
            },
            {
              code: 'stale_context',
              severity: 'warning',
              source: 'task_context',
              otherSource: null,
              message: 'the project changed since part of its Harness was written',
              excerpt: '',
            },
          ],
          staleItems: 2,
        })}
        guardrails={{ ...GUARDRAILS, asked: 1, allowed: 1 }}
      />,
    );

    expect(await screen.findByText('Needs review')).toBeInTheDocument();
    expect(
      screen.getByText('1 warning(s), 1 error(s), 0 blocking, 2 stale item(s)'),
    ).toBeInTheDocument();
    // Closed until clicked; the other side of the conflict is what the click is for.
    expect(screen.queryByText('between Task context and Agent instructions')).not.toBeVisible();
    await user.click(screen.getByText('Two sources disagree'));
    expect(screen.getByText('between Task context and Agent instructions')).toBeVisible();
    expect(
      screen.getByText('two sources disagree on the database: PostgreSQL vs MongoDB'),
    ).toBeVisible();
    expect(screen.getByText('Stack: Use PostgreSQL for all persistence.')).toBeVisible();
    expect(screen.getByText('Part of the context may be outdated')).toBeInTheDocument();
  });

  it('shows a context that could not be sent as blocked', async () => {
    renderWithProviders(
      <ContextReviewPanel
        review={review({
          health: 'invalid',
          issues: [
            {
              code: 'missing_required',
              severity: 'blocking',
              source: 'brief_protocols',
              otherSource: null,
              message: "the step's result protocol is missing",
              excerpt: '',
            },
          ],
        })}
        guardrails={{ ...GUARDRAILS, denied: 1, blocked: 1, allowed: 0 }}
      />,
    );

    expect(await screen.findByText('Blocked')).toBeInTheDocument();
    expect(screen.getByText('A required part is missing')).toBeInTheDocument();
    expect(screen.getByText('Blocking')).toBeInTheDocument();
  });

  it('is in Portuguese for a Portuguese user, and works without the guardrail counters', async () => {
    mockBackend({ language: 'pt-BR' });
    renderWithProviders(
      <ContextReviewPanel review={review({ health: 'partial' })} guardrails={null} />,
    );

    expect(await screen.findByText('Parcial')).toBeInTheDocument();
    expect(screen.getByText('Revisão do contexto'.toUpperCase())).toBeInTheDocument();
    expect(screen.queryByText(/Proteções:/)).not.toBeInTheDocument();
  });
});
