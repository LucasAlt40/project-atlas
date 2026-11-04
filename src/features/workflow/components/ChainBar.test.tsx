import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { noIntegration, passwordRecovery, run } from '@/test/workflowFixtures';
import { ChainBar } from './ChainBar';

const projects = [
  { id: 'back', name: 'API' },
  { id: 'front', name: 'Web' },
];

describe('ChainBar', () => {
  it('says why the chain went no further when the next run could not be started', () => {
    const first = run(
      passwordRecovery(),
      'completed',
      {},
      {
        chain: {
          id: 'wfx-1',
          projects,
          position: 0,
          readProjectIds: [],
          earlier: [],
          stopped: { projectId: 'front', code: 'scope_invalid', params: {} },
        },
      },
    );

    render(<ChainBar run={first} runs={[first]} onShow={vi.fn()} />);

    expect(screen.getByRole('alert')).toHaveTextContent('The chain stopped before Web');
    expect(screen.getByRole('alert')).toHaveTextContent(
      'The scope names a project this workspace does not have.',
    );
    expect(screen.getByRole('button', { name: /Web · not started/ })).toBeDisabled();
  });

  it('shows no alert while the chain goes on', () => {
    const first = run(
      passwordRecovery(),
      'running',
      {},
      {
        chain: { id: 'wfx-1', projects, position: 0, readProjectIds: [], earlier: [] },
      },
    );

    render(<ChainBar run={first} runs={[first]} onShow={vi.fn()} />);

    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Web · waiting/ })).toBeDisabled();
  });

  const completedFirst = (
    extra: Partial<ReturnType<typeof run>['chain'] & object> = {},
    status: 'changes_available' | 'integrated' = 'changes_available',
  ) =>
    run(
      passwordRecovery(),
      'completed',
      {},
      {
        integration: { ...noIntegration(), status },
        chain: { id: 'wfx-1', projects, position: 0, readProjectIds: [], earlier: [], ...extra },
      },
    );

  it('offers to go on in the next project when the chain did not go on by itself', async () => {
    const onContinue = vi.fn();
    const first = completedFirst();

    render(<ChainBar run={first} runs={[first]} onShow={vi.fn()} onContinue={onContinue} />);
    await userEvent.click(screen.getByRole('button', { name: 'Continue in Web' }));

    expect(onContinue).toHaveBeenCalledWith('wfx-1');
  });

  it('says it waits for the code to be delivered instead of offering to go on', () => {
    const first = completedFirst({ waitForDelivery: true });

    render(<ChainBar run={first} runs={[first]} onShow={vi.fn()} onContinue={vi.fn()} />);

    expect(screen.getByRole('status')).toHaveTextContent('applied or kept before Web runs');
    expect(screen.queryByRole('button', { name: 'Continue in Web' })).toBeNull();
  });

  it('offers to go on once the code was applied, and nothing once the next run exists', () => {
    const first = completedFirst({ waitForDelivery: true }, 'integrated');
    const { rerender } = render(
      <ChainBar run={first} runs={[first]} onShow={vi.fn()} onContinue={vi.fn()} />,
    );
    expect(screen.getByRole('button', { name: 'Continue in Web' })).toBeInTheDocument();

    const second = run(
      passwordRecovery(),
      'running',
      {},
      {
        id: 'wfx-2',
        chain: { id: 'wfx-1', projects, position: 1, readProjectIds: [], earlier: [] },
      },
    );
    rerender(<ChainBar run={first} runs={[first, second]} onShow={vi.fn()} onContinue={vi.fn()} />);
    expect(screen.queryByRole('button', { name: 'Continue in Web' })).toBeNull();
  });

  it('does not offer to try again after the code was discarded', () => {
    const first = completedFirst({
      waitForDelivery: true,
      stopped: { projectId: 'front', code: 'chain_changes_discarded', params: {} },
    });

    render(<ChainBar run={first} runs={[first]} onShow={vi.fn()} onContinue={vi.fn()} />);

    expect(screen.getByRole('alert')).toHaveTextContent('was discarded');
    expect(screen.queryByRole('button', { name: /Continue in Web|Try Web again/ })).toBeNull();
  });
});
