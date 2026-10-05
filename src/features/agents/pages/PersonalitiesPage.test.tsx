import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {
  architectPersonality,
  mockBackend,
  personalities,
  renderWithProviders,
} from '@/test/fixtures';
import {
  createPersonality,
  deletePersonality,
  restoreDefaultPersonalities,
  updatePersonality,
} from '../services/catalogService';
import { PersonalitiesPage } from './PersonalitiesPage';

vi.mock('../services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');

function show(onUse = vi.fn(), options = {}) {
  mockBackend(options);
  renderWithProviders(<PersonalitiesPage onUse={onUse} />);
  return onUse;
}

describe('PersonalitiesPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows what a personality does, including its instructions, before it is used', async () => {
    show();

    const card = await screen.findByRole('article', { name: 'Architect' });
    expect(within(card).getByText('Identifies boundaries')).toBeInTheDocument();
    expect(within(card).getByLabelText('Architect system instructions')).toHaveTextContent(
      'You are an experienced software architect.',
    );
    // The protocol Atlas adds to every personality is stated once, above the cards.
    expect(
      screen.getByText(/live-narration and read-only rules to every personality/),
    ).toBeInTheDocument();
    expect(screen.getByRole('article', { name: 'QA' })).toBeInTheDocument();
  });

  it('hands the chosen personality to the "create agent" flow', async () => {
    const user = userEvent.setup();
    const onUse = show();

    await user.click(
      within(await screen.findByRole('article', { name: 'QA' })).getByRole('button', {
        name: 'Use this personality',
      }),
    );

    expect(onUse).toHaveBeenCalledWith('qa');
  });

  it('saves a custom personality', async () => {
    const user = userEvent.setup();
    show();
    vi.mocked(createPersonality).mockResolvedValue({
      id: 'custom-1',
      name: 'Senior .NET Architect',
      description: 'DDD',
      systemInstructions: 'You are a senior .NET architect.',
      behavior: [],
      tags: ['ddd'],
      source: 'custom',
      suggestedContract: { kind: 'general', outcomes: [] },
      suggestedPermissionProfile: 'developer',
    });

    await user.click(await screen.findByRole('button', { name: 'New personality' }));
    await user.type(screen.getByLabelText('Name'), 'Senior .NET Architect');
    await user.type(screen.getByLabelText('Instructions'), 'You are a senior .NET architect.');
    await user.type(screen.getByLabelText('Tags', { selector: 'input' }), 'ddd, clean');
    await user.click(screen.getByRole('button', { name: 'Save personality' }));

    expect(createPersonality).toHaveBeenCalledWith({
      name: 'Senior .NET Architect',
      description: '',
      systemInstructions: 'You are a senior .NET architect.',
      tags: ['ddd', ' clean'],
    });
    expect(
      await screen.findByRole('article', { name: 'Senior .NET Architect' }),
    ).toBeInTheDocument();
  });

  it('words a validation error from the core in the interface language', async () => {
    const user = userEvent.setup();
    show();
    vi.mocked(createPersonality).mockRejectedValue({
      code: 'name_required',
      params: {},
      detail: null,
    });

    await user.click(await screen.findByRole('button', { name: 'New personality' }));
    await user.click(screen.getByRole('button', { name: 'Save personality' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Give it a name.');
  });

  it('edits a personality and shows the saved version', async () => {
    const user = userEvent.setup();
    show();
    vi.mocked(updatePersonality).mockResolvedValue({
      ...architectPersonality,
      name: 'Strict Architect',
      systemInstructions: 'Be strict.',
      behavior: [],
    });

    await user.click(await screen.findByRole('button', { name: 'Edit Architect' }));
    const form = screen.getByRole('form', { name: 'Edit Architect' });
    expect(within(form).getByLabelText('Instructions')).toHaveValue(
      'You are an experienced software architect.',
    );
    expect(within(form).getByText(/built-in personality/)).toBeInTheDocument();
    await user.clear(within(form).getByLabelText('Name'));
    await user.type(within(form).getByLabelText('Name'), 'Strict Architect');
    await user.clear(within(form).getByLabelText('Instructions'));
    await user.type(within(form).getByLabelText('Instructions'), 'Be strict.');
    await user.click(within(form).getByRole('button', { name: 'Save personality' }));

    expect(updatePersonality).toHaveBeenCalledWith('architect', {
      name: 'Strict Architect',
      description: 'Architecture-focused agent profile.',
      systemInstructions: 'Be strict.',
      tags: ['architecture'],
    });
    expect(await screen.findByRole('article', { name: 'Strict Architect' })).toBeInTheDocument();
  });

  it('deletes a personality only after confirmation, and keeps one that agents use, saying why', async () => {
    const user = userEvent.setup();
    show();
    vi.mocked(deletePersonality)
      .mockResolvedValueOnce()
      .mockRejectedValueOnce({
        code: 'personality_in_use',
        params: { name: 'Architect', agents: 'Architecture Expert' },
        detail: null,
      });

    await user.click(await screen.findByRole('button', { name: 'Delete QA' }));
    expect(deletePersonality).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Confirm delete' }));
    await waitFor(() => {
      expect(screen.queryByRole('article', { name: 'QA' })).not.toBeInTheDocument();
    });

    await user.click(screen.getByRole('button', { name: 'Delete Architect' }));
    await user.click(screen.getByRole('button', { name: 'Confirm delete' }));
    const card = screen.getByRole('article', { name: 'Architect' });
    expect(await within(card).findByRole('alert')).toHaveTextContent(
      'Architect is used by Architecture Expert. Change or delete the agents first.',
    );
  });

  it('restores the default personalities', async () => {
    const user = userEvent.setup();
    show(vi.fn(), { personalities: [architectPersonality] });
    vi.mocked(restoreDefaultPersonalities).mockResolvedValue(personalities);

    await user.click(await screen.findByRole('button', { name: 'Restore default personalities' }));

    expect(await screen.findByRole('article', { name: 'QA' })).toBeInTheDocument();
  });
});
