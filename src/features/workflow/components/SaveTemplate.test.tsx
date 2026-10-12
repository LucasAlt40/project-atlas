import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { I18nProvider } from '@/i18n/I18nProvider';
import { SaveTemplate } from './SaveTemplate';

function show(dirty: boolean) {
  const onSave = vi.fn();
  const onCancel = vi.fn();
  render(
    <I18nProvider language="en-US">
      <SaveTemplate
        dirty={dirty}
        defaultName="Password recovery"
        onSave={onSave}
        onCancel={onCancel}
      />
    </I18nProvider>,
  );
  return { onSave, onCancel };
}

describe('saving a workflow as a template', () => {
  it('saves under the name and description given, and says the built-in templates stay as they are', async () => {
    const user = userEvent.setup();
    const { onSave } = show(false);
    expect(screen.getByText(/built-in templates are not changed/)).toBeInTheDocument();

    await user.clear(screen.getByLabelText('Template name'));
    await user.type(screen.getByLabelText('Template name'), 'House rules');
    await user.type(screen.getByLabelText('Description (optional)'), 'How we do it');
    await user.click(screen.getByRole('button', { name: 'Save template' }));

    expect(onSave).toHaveBeenCalledWith('House rules', 'How we do it');
  });

  it('waits for the workflow to be saved, because the template is made from the saved version', () => {
    const { onSave } = show(true);

    expect(screen.getByRole('status')).toHaveTextContent('Save the workflow first');
    expect(screen.getByRole('button', { name: 'Save template' })).toBeDisabled();
    expect(onSave).not.toHaveBeenCalled();
  });
});
