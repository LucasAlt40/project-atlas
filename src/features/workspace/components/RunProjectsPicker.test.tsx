import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { renderWithProviders } from '@/test/fixtures';
import { allWorking, type RunPlan } from '../model/runPlan';
import { RunProjectsPicker } from './RunProjectsPicker';

const project = (id: string) => ({ id, name: id, path: `/${id}`, role: null, dependsOn: [] });
const projects = [project('api'), project('web'), project('admin')];

function Harness({ onPlan }: { onPlan: (plan: RunPlan) => void }) {
  const [plan, setPlan] = useState(allWorking({ projects }));
  return (
    <RunProjectsPicker
      projects={projects}
      plan={plan}
      onChange={(next) => {
        setPlan(next);
        onPlan(next);
      }}
    />
  );
}

function order() {
  const list = screen.getAllByRole('list')[0];
  if (!list) throw new Error('no list');
  return within(list)
    .getAllByRole('listitem')
    .map((item) => item.querySelector('span span')?.textContent);
}

beforeAll(() => {
  // jsdom has no pointer capture; the browser does.
  HTMLElement.prototype.setPointerCapture = () => undefined;
});

describe('choosing the order of the runs', () => {
  it('moves a project by dragging its handle to where it is dropped', async () => {
    const onPlan = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(<Harness onPlan={onPlan} />);
    await user.click(await screen.findByRole('button', { name: /^Projetos da execução/ }));
    const [list] = screen.getAllByRole('list');
    if (!list) throw new Error('no list');
    const rows = within(list).getAllByRole('listitem');
    // Rows laid out 40px apart, as the browser would.
    rows.forEach((row, index) => {
      row.getBoundingClientRect = () =>
        ({ top: index * 40, height: 40, bottom: index * 40 + 40 }) as DOMRect;
    });
    const handle = screen.getByRole('button', { name: /^Arrastar api/ });

    fireEvent.pointerDown(handle, { button: 0, pointerId: 1, clientY: 20 });
    fireEvent.pointerMove(handle, { pointerId: 1, clientY: 110 });
    expect(order()).toEqual(['web', 'admin', 'api']);
    fireEvent.pointerUp(handle, { pointerId: 1, clientY: 110 });

    expect(onPlan).toHaveBeenLastCalledWith({
      workingProjectIds: ['web', 'admin', 'api'],
      readProjectIds: [],
    });
  });

  it('moves a project with the arrow keys on its handle', async () => {
    const onPlan = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(<Harness onPlan={onPlan} />);
    await user.click(await screen.findByRole('button', { name: /^Projetos da execução/ }));

    screen.getByRole('button', { name: /^Arrastar admin/ }).focus();
    await user.keyboard('{ArrowUp}');

    expect(order()).toEqual(['api', 'admin', 'web']);
  });

  it('lists the projects that do not change apart, without a handle', async () => {
    const user = userEvent.setup();
    renderWithProviders(<Harness onPlan={vi.fn()} />);
    await user.click(await screen.findByRole('button', { name: /^Projetos da execução/ }));

    const web = screen.getByRole('group', { name: 'Uso de web' });
    await user.click(within(web).getByRole('button', { name: 'Consulta' }));

    expect(screen.getByText('Outros projetos')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^Arrastar web/ })).not.toBeInTheDocument();
    expect(order()).toEqual(['api', 'admin']);
  });
});
