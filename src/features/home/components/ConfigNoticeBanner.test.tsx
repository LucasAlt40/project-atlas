import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { fetchAppInfo } from '../services/appInfoService';
import { ConfigNoticeBanner } from './ConfigNoticeBanner';

vi.mock('../services/appInfoService');

const base = { name: 'Project Atlas', version: '0.3.0', platform: 'macos' };

describe('ConfigNoticeBanner', () => {
  it('shows nothing when the config was read as it was', async () => {
    vi.mocked(fetchAppInfo).mockResolvedValue(base);
    const { container } = render(<ConfigNoticeBanner />);
    await vi.waitFor(() => {
      expect(fetchAppInfo).toHaveBeenCalled();
    });
    expect(container).toBeEmptyDOMElement();
  });

  it('says which records were left out and where the original file is, until dismissed', async () => {
    vi.mocked(fetchAppInfo).mockResolvedValue({
      ...base,
      configNotice: {
        kind: 'recovered',
        backup: '/data/config.json.corrupt',
        dropped: [{ section: 'workflowExecutions', count: 2 }],
      },
    });
    render(<ConfigNoticeBanner />);

    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('workflowExecutions: 2');
    expect(alert).toHaveTextContent('/data/config.json.corrupt');

    await userEvent.click(screen.getByRole('button', { name: 'Dismiss' }));
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('warns that nothing is saved when the file could not be opened', async () => {
    vi.mocked(fetchAppInfo).mockResolvedValue({
      ...base,
      configNotice: { kind: 'unreadable', path: '/data/config.json' },
    });
    render(<ConfigNoticeBanner />);
    expect(await screen.findByRole('alert')).toHaveTextContent('Nothing is saved');
  });
});
