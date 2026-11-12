import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { I18nProvider } from '@/i18n/I18nProvider';
import { ParallelStepsPicker } from './ParallelStepsPicker';

describe('ParallelStepsPicker', () => {
  it('offers one to eight steps, shows the run’s value and says what the person chose', async () => {
    const onChange = vi.fn();
    render(
      <I18nProvider language="en-US">
        <ParallelStepsPicker value={4} onChange={onChange} />
      </I18nProvider>,
    );

    const picker = screen.getByRole('combobox', { name: 'Steps in parallel' });
    expect(picker).toHaveValue('4');
    expect(screen.getAllByRole('option').map((o) => o.textContent)).toEqual([
      '1',
      '2',
      '3',
      '4',
      '5',
      '6',
      '7',
      '8',
    ]);

    await userEvent.selectOptions(picker, '2');

    expect(onChange).toHaveBeenCalledWith(2);
  });
});
