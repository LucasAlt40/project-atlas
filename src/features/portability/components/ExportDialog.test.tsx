import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { CommandError, type ExportedFileDto } from '@/lib/tauri/commands';
import { mockBackend, renderWithProviders } from '@/test/fixtures';
import { exportConfigs, saveConfigFile } from '../services/portabilityService';
import { ExportButton } from './PortabilityActions';

vi.mock('../services/portabilityService');
vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');

const FILE: ExportedFileDto = {
  fileName: 'architect.atlas.json',
  text: '{"format":"atlas-config"}',
  items: [
    { kind: 'agent', name: 'Architect', chosen: true },
    { kind: 'personality', name: 'Reviewer', chosen: false },
  ],
  external: [],
  warnings: [],
};

async function open(file: ExportedFileDto = FILE) {
  mockBackend({});
  vi.mocked(exportConfigs).mockResolvedValue(file);
  const user = userEvent.setup();
  renderWithProviders(<ExportButton kind="agent" id="agent-1" name="Architect" />);
  await user.click(await screen.findByRole('button', { name: 'Export Architect' }));
  const dialog = await screen.findByRole('dialog', { name: 'Export “Architect”' });
  await within(dialog).findByText('architect.atlas.json');
  return { user, dialog };
}

describe('ExportDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('exports what was chosen and shows what the file will hold, and what it never does', async () => {
    const { dialog } = await open();

    expect(exportConfigs).toHaveBeenCalledWith([{ kind: 'agent', id: 'agent-1' }], true);
    expect(
      within(dialog).getByText(/never holds API keys, tokens, credentials/),
    ).toBeInTheDocument();
    const contents = within(dialog).getByRole('list');
    expect(within(contents).getByText('Architect')).toBeInTheDocument();
    expect(within(contents).getByText('Reviewer')).toBeInTheDocument();
    expect(within(contents).getByText('Needed by another')).toBeInTheDocument();
  });

  it('asks the core again, without the dependencies, and lists what is only named', async () => {
    const { user, dialog } = await open();
    vi.mocked(exportConfigs).mockResolvedValue({
      ...FILE,
      items: [{ kind: 'agent', name: 'Architect', chosen: true }],
      external: [{ kind: 'personality', name: 'Reviewer', chosen: false }],
    });

    await user.click(within(dialog).getByRole('checkbox', { name: /Include what it needs/ }));

    await waitFor(() => {
      expect(exportConfigs).toHaveBeenLastCalledWith([{ kind: 'agent', id: 'agent-1' }], false);
    });
    expect(await within(dialog).findByText('Named, not included')).toBeInTheDocument();
    expect(
      within(dialog).getByText(/Whoever imports the file will be asked which of their own/),
    ).toBeInTheDocument();
  });

  it('warns about what is worth a second look before the file is saved', async () => {
    const { dialog } = await open({
      ...FILE,
      warnings: [
        { code: 'possible_secret', name: 'Deployer' },
        { code: 'agent_missing', name: 'Feature flow' },
      ],
    });

    const warnings = within(dialog).getByRole('list', { name: 'Worth a second look' });
    expect(
      within(warnings).getByText(
        '“Deployer” has a text that looks like a credential. Check it before sharing the file.',
      ),
    ).toBeInTheDocument();
    expect(
      within(warnings).getByText(/“Feature flow” has a step whose agent no longer exists/),
    ).toBeInTheDocument();
  });

  it('saves through the operating system’s dialog and says where, or that nothing was saved', async () => {
    const { user, dialog } = await open();
    vi.mocked(saveConfigFile).mockResolvedValueOnce('chosen-name.json');

    await user.click(within(dialog).getByRole('button', { name: 'Save file…' }));

    expect(saveConfigFile).toHaveBeenCalledWith(FILE.fileName, FILE.text);
    expect(await within(dialog).findByRole('status')).toHaveTextContent(
      'Saved as chosen-name.json.',
    );
    vi.mocked(saveConfigFile).mockResolvedValueOnce(null);
    await user.click(within(dialog).getByRole('button', { name: 'Save file…' }));
    expect(await within(dialog).findByText('Nothing was saved.')).toBeInTheDocument();
  });

  it('says so when the core cannot export what was chosen, and does not offer to save it', async () => {
    mockBackend({});
    vi.mocked(exportConfigs).mockRejectedValue(
      new CommandError('export_configs', {
        code: 'export_target_not_found',
        params: {},
        detail: null,
      }),
    );
    const user = userEvent.setup();
    renderWithProviders(<ExportButton kind="agent" id="gone" name="Gone" />);

    await user.click(await screen.findByRole('button', { name: 'Export Gone' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'What you chose to export no longer exists.',
    );
    expect(screen.getByRole('button', { name: 'Save file…' })).toBeDisabled();
  });

  it('shows a failed save without losing the file', async () => {
    const { user, dialog } = await open();
    vi.mocked(saveConfigFile).mockRejectedValue(
      new CommandError('save_config_file', {
        code: 'config_file_io',
        params: {},
        detail: 'denied',
      }),
    );

    await user.click(within(dialog).getByRole('button', { name: 'Save file…' }));

    expect(await within(dialog).findByRole('alert')).toHaveTextContent(
      'The file could not be read or written.',
    );
    expect(within(dialog).getByRole('button', { name: 'Save file…' })).toBeEnabled();
  });
});
