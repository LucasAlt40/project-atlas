import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ExternalLink } from './ExternalLink';

describe('ExternalLink', () => {
  it('shows only https addresses', () => {
    const { container } = render(<ExternalLink href="javascript:alert(1)">Source</ExternalLink>);
    expect(container).toBeEmptyDOMElement();
  });

  it('copies the address instead of navigating', async () => {
    const user = userEvent.setup();
    const writeText = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
    render(<ExternalLink href="https://github.com/acme/jira">Source</ExternalLink>);

    const link = screen.getByRole('link', { name: /Source/ });
    expect(link).toHaveAttribute('title', 'https://github.com/acme/jira');
    expect(link).not.toHaveAttribute('target');
    await user.click(link);

    expect(writeText).toHaveBeenCalledWith('https://github.com/acme/jira');
    expect(await screen.findByText(/Address copied/)).toBeInTheDocument();
  });
});
