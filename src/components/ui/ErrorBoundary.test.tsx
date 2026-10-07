import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { I18nProvider } from '@/i18n/I18nProvider';
import { ErrorBoundary } from './ErrorBoundary';

let broken = true;

function Screen() {
  if (broken) throw new Error('render failed');
  return <p>all good</p>;
}

describe('ErrorBoundary', () => {
  afterEach(() => {
    broken = true;
  });

  it('shows a message instead of a blank window and lets the person try again', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined);
    render(
      <I18nProvider language="en-US">
        <ErrorBoundary>
          <Screen />
        </ErrorBoundary>
      </I18nProvider>,
    );

    expect(screen.getByRole('alert')).toBeInTheDocument();

    broken = false;
    await userEvent.click(screen.getByRole('button', { name: 'Try again' }));

    expect(screen.getByText('all good')).toBeInTheDocument();
  });
});
