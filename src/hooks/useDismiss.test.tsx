import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useRef, useState } from 'react';
import { useDismiss } from './useDismiss';

function Popover() {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  useDismiss(open, root, () => {
    setOpen(false);
  });
  return (
    <div>
      <div ref={root}>
        <button
          type="button"
          onClick={() => {
            setOpen((o) => !o);
          }}
        >
          Abrir
        </button>
        {open && <p>Conteúdo</p>}
      </div>
      <button type="button">Fora</button>
      <div
        data-testid="canvas"
        onPointerDown={(event) => {
          event.stopPropagation();
        }}
      />
    </div>
  );
}

describe('useDismiss', () => {
  it('closes on a click outside, even where the event is stopped, and on Escape', async () => {
    const user = userEvent.setup();
    render(<Popover />);

    await user.click(screen.getByRole('button', { name: 'Abrir' }));
    expect(screen.getByText('Conteúdo')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Fora' }));
    expect(screen.queryByText('Conteúdo')).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Abrir' }));
    await user.click(screen.getByTestId('canvas'));
    expect(screen.queryByText('Conteúdo')).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Abrir' }));
    await user.keyboard('{Escape}');
    expect(screen.queryByText('Conteúdo')).not.toBeInTheDocument();
  });

  it('stays open for a click inside', async () => {
    const user = userEvent.setup();
    render(<Popover />);
    await user.click(screen.getByRole('button', { name: 'Abrir' }));
    await user.click(screen.getByText('Conteúdo'));
    expect(screen.getByText('Conteúdo')).toBeInTheDocument();
  });
});
