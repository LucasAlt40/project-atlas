import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { mockBackend, renderWithProviders } from '@/test/fixtures';
import type { MemoryDto } from '@/lib/tauri/commands';
import { setMemorySettings } from '@/features/settings/services/settingsService';
import {
  approveMemory,
  createMemory,
  deleteAutomaticMemories,
  deleteMemory,
  listMemories,
  promoteMemory,
  setMemoryStatus,
} from '../services/memoryService';
import { MemorySettings } from './MemorySettings';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');
vi.mock('@/features/home/services/appInfoService');
vi.mock('@/features/portability/services/portabilityService');
vi.mock('@/features/workflow/services/workflowService');
vi.mock('../services/memoryService');

function note(overrides: Partial<MemoryDto> = {}): MemoryDto {
  return {
    id: 'm1',
    scope: 'workspace',
    owner: 'w1',
    kind: 'pitfall',
    title: 'Invoice rounding',
    content: 'Totals round half even, never use floats',
    provenance: { origin: 'user', automatic: false },
    status: 'active',
    validity: { paths: [], scripts: [] },
    uses: 2,
    createdAt: 1,
    updatedAt: 1,
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  mockBackend({ language: 'en-US' });
  vi.mocked(listMemories).mockResolvedValue([]);
});

describe('MemorySettings', () => {
  it('starts with every scope off and keeps the user scope free of automatic mode', async () => {
    renderWithProviders(<MemorySettings />);

    const user = await screen.findByLabelText('Mine (every project)');
    expect(user).toHaveValue('off');
    expect(
      within(user)
        .getAllByRole('option')
        .map((o) => o.textContent),
    ).toEqual(['Off', 'Propose, I approve']);
    const workspace = screen.getByLabelText('Workspace');
    expect(
      within(workspace)
        .getAllByRole('option')
        .map((o) => o.textContent),
    ).toEqual(['Off', 'Propose, I approve', 'Automatic']);
  });

  it('saves the mode of a scope without touching the others', async () => {
    const person = userEvent.setup();
    vi.mocked(setMemorySettings).mockImplementation((memory) =>
      Promise.resolve({
        language: 'en-US',
        selectedWorkspaceId: null,
        optimization: { metricsEnabled: true, memory },
      }),
    );
    renderWithProviders(<MemorySettings />);

    await person.selectOptions(await screen.findByLabelText('Workspace'), 'auto');

    expect(setMemorySettings).toHaveBeenCalledWith({
      user: 'off',
      workspace: 'auto',
      agent: 'off',
      maxTokens: 600,
    });
    expect(await screen.findByLabelText('Workspace')).toHaveValue('auto');
  });

  it('lists what waits for a decision apart, and approving or discarding acts on it', async () => {
    const person = userEvent.setup();
    vi.mocked(listMemories).mockResolvedValue([
      note({
        id: 'c1',
        title: 'Run the check',
        status: 'candidate',
        provenance: { origin: 'agent', automatic: false },
      }),
      note(),
    ]);
    vi.mocked(approveMemory).mockResolvedValue(note({ id: 'c1', title: 'Run the check' }));
    vi.mocked(deleteMemory).mockResolvedValue(null);
    renderWithProviders(<MemorySettings />);

    const waiting = await screen.findByRole('region', { name: 'Waiting for your decision' });
    expect(within(waiting).getByText('Run the check')).toBeInTheDocument();
    expect(within(waiting).getByText('Proposed by an agent')).toBeInTheDocument();
    expect(within(waiting).queryByText('Invoice rounding')).not.toBeInTheDocument();

    await person.click(within(waiting).getByRole('button', { name: 'Approve' }));
    expect(approveMemory).toHaveBeenCalledWith('c1');
    // Approved: it left the waiting list and is a kept note now.
    expect(
      screen.queryByRole('region', { name: 'Waiting for your decision' }),
    ).not.toBeInTheDocument();
    expect(screen.getByText('Run the check')).toBeInTheDocument();
  });

  it('discarding a proposal deletes it', async () => {
    const person = userEvent.setup();
    vi.mocked(listMemories).mockResolvedValue([
      note({ id: 'c1', status: 'candidate', provenance: { origin: 'agent', automatic: false } }),
    ]);
    vi.mocked(deleteMemory).mockResolvedValue(null);
    renderWithProviders(<MemorySettings />);

    await person.click(await screen.findByRole('button', { name: 'Discard' }));

    expect(deleteMemory).toHaveBeenCalledWith('c1');
    expect(
      screen.queryByRole('region', { name: 'Waiting for your decision' }),
    ).not.toBeInTheDocument();
  });

  it('marks an automatic note, lets the person make it theirs and clear all of them', async () => {
    const person = userEvent.setup();
    vi.mocked(listMemories).mockResolvedValue([
      note({ provenance: { origin: 'agent', automatic: true } }),
    ]);
    vi.mocked(promoteMemory).mockResolvedValue(note());
    vi.mocked(deleteAutomaticMemories).mockResolvedValue(1);
    renderWithProviders(<MemorySettings />);

    expect(
      await screen.findByText('Unconfirmed (automatic)', { selector: 'span' }),
    ).toBeInTheDocument();
    await person.click(screen.getByRole('button', { name: 'Make it mine' }));
    expect(promoteMemory).toHaveBeenCalledWith('m1');
    expect(
      screen.queryByText('Unconfirmed (automatic)', { selector: 'span' }),
    ).not.toBeInTheDocument();
  });

  it('can set a stale note aside or use it again', async () => {
    const person = userEvent.setup();
    vi.mocked(listMemories).mockResolvedValue([note({ status: 'stale' })]);
    vi.mocked(setMemoryStatus).mockResolvedValue(note());
    renderWithProviders(<MemorySettings />);

    expect(await screen.findByText('Out of date')).toBeInTheDocument();
    await person.click(screen.getByRole('button', { name: 'Use again' }));

    expect(setMemoryStatus).toHaveBeenCalledWith('m1', 'active');
  });

  it('adds a note of the user’s own scope', async () => {
    const person = userEvent.setup();
    vi.mocked(createMemory).mockResolvedValue(note({ scope: 'user', owner: null }));
    renderWithProviders(<MemorySettings />);

    await person.type(await screen.findByLabelText('Title'), 'Package manager');
    await person.type(screen.getByLabelText('Lesson'), 'Use pnpm here');
    await person.click(screen.getByRole('button', { name: 'Add note' }));

    expect(createMemory).toHaveBeenCalledWith({
      scope: 'user',
      owner: null,
      kind: 'how_to',
      title: 'Package manager',
      content: 'Use pnpm here',
    });
    expect(screen.getByLabelText('Title')).toHaveValue('');
  });

  it('says why a note was refused, in the person’s language', async () => {
    const person = userEvent.setup();
    vi.mocked(createMemory).mockRejectedValue({
      code: 'memory_looks_like_secret',
      params: {},
      detail: null,
    });
    renderWithProviders(<MemorySettings />);

    await person.type(await screen.findByLabelText('Title'), 'Login');
    await person.type(screen.getByLabelText('Lesson'), 'a key');
    await person.click(screen.getByRole('button', { name: 'Add note' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('looks like a secret');
  });

  it('shows a note held for a validator as waiting, and one a validator let in as unconfirmed but validated', async () => {
    vi.mocked(listMemories).mockResolvedValue([
      note({
        id: 'held',
        title: 'Held lesson',
        status: 'candidate',
        provenance: { origin: 'agent', automatic: true, agentId: 'a1' },
      }),
      note({
        id: 'in',
        title: 'Let in lesson',
        provenance: { origin: 'agent', automatic: true, validatedBy: 'exec-9' },
      }),
    ]);
    renderWithProviders(<MemorySettings />);

    const waiting = await screen.findByRole('region', { name: 'Waiting for your decision' });
    expect(within(waiting).getByText('Held lesson')).toBeInTheDocument();
    expect(within(waiting).getByText('Waiting for an independent validator')).toBeInTheDocument();
    expect(within(waiting).queryByText('Proposed by an agent')).not.toBeInTheDocument();

    const kept = screen.getByRole('region', { name: 'Kept notes' });
    const row = within(kept).getByText('Let in lesson').closest('li');
    expect(row).toHaveTextContent('Unconfirmed (automatic)');
    expect(row).toHaveTextContent('Passed by another agent’s validation');
  });
});
