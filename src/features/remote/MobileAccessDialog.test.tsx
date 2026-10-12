import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { RemoteStatusDto } from '@/lib/tauri/commands';
import { mockBackend, renderWithProviders } from '@/test/fixtures';
import { MobileAccessButton } from './MobileAccessButton';
import {
  getRemoteStatus,
  newPairingCode,
  revokeAllDevices,
  revokeDevice,
  setRemoteEnabled,
} from './remoteService';

vi.mock('./remoteService');
vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');

const OFF: RemoteStatusDto = {
  state: 'disabled',
  error: null,
  url: null,
  address: null,
  port: null,
  addressLost: false,
  candidates: [
    { address: '192.168.1.20', interface: 'en0', kind: 'lan', recommended: true },
    { address: '10.8.0.2', interface: 'utun3', kind: 'lan', recommended: false },
  ],
  pairing: null,
  devices: [],
  waiting: 0,
  serverTime: 1_000_000,
};

const ON: RemoteStatusDto = {
  ...OFF,
  state: 'active',
  url: 'http://192.168.1.20:47615',
  address: '192.168.1.20',
  port: 47615,
  pairing: {
    code: 'ABCD-EFGH-IJKL-MNOP',
    url: 'http://192.168.1.20:47615/#ABCDEFGHIJKLMNOP',
    qrSvg: '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"></svg>',
    expiresAt: 1_000_000 + 4 * 60_000 + 30_000,
  },
};

async function open(status: RemoteStatusDto) {
  mockBackend({});
  vi.mocked(getRemoteStatus).mockResolvedValue(status);
  const user = userEvent.setup();
  renderWithProviders(<MobileAccessButton />);
  await user.click(await screen.findByRole('button', { name: /Phone/ }));
  return { user, dialog: await screen.findByRole('dialog', { name: 'Mobile access' }) };
}

describe('Mobile access', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('is off until the person turns it on, and says what it is', async () => {
    const { dialog } = await open(OFF);

    expect(within(dialog).getByText('Off')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Turn on' })).toBeEnabled();
    expect(within(dialog).queryByRole('img')).not.toBeInTheDocument();
    expect(
      within(dialog).getByText(/Nothing goes through the internet or a cloud/),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByText(/only works while this computer is on, awake/),
    ).toBeInTheDocument();
    expect(within(dialog).getByText(/plain HTTP on your local network/)).toBeInTheDocument();
    expect(setRemoteEnabled).not.toHaveBeenCalled();
  });

  it('offers the addresses the computer has and starts on the recommended one', async () => {
    const { user, dialog } = await open(OFF);
    vi.mocked(setRemoteEnabled).mockResolvedValue(ON);

    const select = within(dialog).getByLabelText('Address on this network');
    expect(select).toHaveValue('192.168.1.20');
    expect(within(select).getAllByRole('option')).toHaveLength(2);
    await user.click(within(dialog).getByRole('button', { name: 'Turn on' }));

    expect(setRemoteEnabled).toHaveBeenCalledWith(true, '192.168.1.20');
    expect(await within(dialog).findByText('On')).toBeInTheDocument();
  });

  it('starts on the address the person picked', async () => {
    const { user, dialog } = await open(OFF);
    vi.mocked(setRemoteEnabled).mockResolvedValue(ON);

    await user.selectOptions(within(dialog).getByLabelText('Address on this network'), '10.8.0.2');
    await user.click(within(dialog).getByRole('button', { name: 'Turn on' }));

    expect(setRemoteEnabled).toHaveBeenCalledWith(true, '10.8.0.2');
  });

  it('shows the QR from the real address, the code, how long it lasts and the address to type', async () => {
    const { dialog } = await open(ON);

    expect(within(dialog).getByRole('img', { name: /QR code/ })).toHaveAttribute(
      'src',
      expect.stringContaining('data:image/svg+xml'),
    );
    expect(within(dialog).getByText('ABCD-EFGH-IJKL-MNOP')).toBeInTheDocument();
    expect(within(dialog).getByText('Expires in 4:30')).toBeInTheDocument();
    expect(within(dialog).getByText('http://192.168.1.20:47615')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Turn off' })).toBeInTheDocument();
    // Once it is on, the address is not a choice any more.
    expect(within(dialog).queryByLabelText('Address on this network')).not.toBeInTheDocument();
  });

  it('turns off in one click', async () => {
    const { user, dialog } = await open(ON);
    vi.mocked(setRemoteEnabled).mockResolvedValue(OFF);

    await user.click(within(dialog).getByRole('button', { name: 'Turn off' }));

    expect(setRemoteEnabled).toHaveBeenCalledWith(false, null);
    expect(await within(dialog).findByText('Off')).toBeInTheDocument();
  });

  it('asks for a new code, and says when there is none', async () => {
    const { user, dialog } = await open({ ...ON, pairing: null });
    expect(within(dialog).getByText(/No pairing code is active/)).toBeInTheDocument();
    vi.mocked(newPairingCode).mockResolvedValue(ON);

    await user.click(within(dialog).getByRole('button', { name: 'New code' }));

    expect(newPairingCode).toHaveBeenCalled();
    expect(await within(dialog).findByText('ABCD-EFGH-IJKL-MNOP')).toBeInTheDocument();
  });

  it('lists who is connected and disconnects one phone or all of them', async () => {
    const devices = [
      { id: 'd1', name: 'Pixel', address: '192.168.1.30', pairedAt: 1, lastSeen: 1_000_000 },
      { id: 'd2', name: 'iPhone', address: '192.168.1.31', pairedAt: 2, lastSeen: 1_000_000 },
    ];
    const { user, dialog } = await open({ ...ON, devices });
    expect(within(dialog).getByText('Pixel')).toBeInTheDocument();
    expect(within(dialog).getByText('iPhone')).toBeInTheDocument();
    vi.mocked(revokeDevice).mockResolvedValue({ ...ON, devices: devices.slice(1) });
    vi.mocked(revokeAllDevices).mockResolvedValue(ON);

    const row = within(dialog)
      .getAllByRole('listitem')
      .find((li) => within(li).queryByText('Pixel'));
    if (!row) throw new Error('the Pixel row is missing');
    await user.click(within(row).getByRole('button', { name: 'Disconnect' }));
    expect(revokeDevice).toHaveBeenCalledWith('d1');
    await waitFor(() => {
      expect(within(dialog).queryByText('Pixel')).not.toBeInTheDocument();
    });
  });

  it('says why it could not start', async () => {
    const { dialog } = await open({ ...OFF, state: 'error', error: 'port_unavailable' });
    expect(within(dialog).getByRole('alert')).toHaveTextContent(
      'ports Atlas uses for this are all taken',
    );
  });

  it('says there is no network when the computer has no address to offer', async () => {
    const { dialog } = await open({ ...OFF, candidates: [] });
    expect(within(dialog).getByText(/No network address was found/)).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Turn on' })).toBeDisabled();
  });

  it('warns that the address changed and what to do', async () => {
    const { dialog } = await open({ ...ON, addressLost: true });
    expect(within(dialog).getByRole('alert')).toHaveTextContent('network address changed');
  });

  it('shows in the header whether it is on and how many questions wait', async () => {
    mockBackend({});
    vi.mocked(getRemoteStatus).mockResolvedValue({ ...ON, waiting: 2 });
    renderWithProviders(<MobileAccessButton />);

    const button = await screen.findByRole('button', { name: /Phone/ });
    await waitFor(() => {
      expect(button).toHaveTextContent('2');
    });
    expect(button).toHaveTextContent('On');
  });
});
