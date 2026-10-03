import { fireEvent, render, screen } from '@testing-library/react';
import { Markdown } from './Markdown';

describe('Markdown', () => {
  it('renders headings, emphasis, lists and code', () => {
    render(
      <Markdown>
        {'## Findings\n\n**Bold** and `code`\n\n- one\n- two\n\n```\nlet x = 1;\n```'}
      </Markdown>,
    );

    expect(screen.getByRole('heading', { name: 'Findings' })).toBeInTheDocument();
    expect(screen.getByText('Bold').tagName).toBe('STRONG');
    expect(screen.getByText('code').tagName).toBe('CODE');
    expect(screen.getAllByRole('listitem')).toHaveLength(2);
    expect(screen.getByText('let x = 1;').closest('pre')).not.toBeNull();
  });

  it('renders GitHub-flavoured tables', () => {
    render(<Markdown>{'| a | b |\n|---|---|\n| 1 | 2 |'}</Markdown>);

    expect(screen.getByRole('table')).toBeInTheDocument();
    expect(screen.getByRole('columnheader', { name: 'a' })).toBeInTheDocument();
  });

  it('never renders raw HTML from the model', () => {
    const { container } = render(
      <Markdown>{'<script>alert(1)</script><img src=x onerror=alert(1)>'}</Markdown>,
    );

    expect(container.querySelector('script')).toBeNull();
    expect(container.querySelector('img')).toBeNull();
  });

  it('does not let links navigate the app', () => {
    render(<Markdown>{'[docs](https://example.com)'}</Markdown>);

    const notPrevented = fireEvent.click(screen.getByRole('link', { name: 'docs' }));

    expect(notPrevented).toBe(false);
  });
});
