import {
  allWorking,
  isChain,
  loadRunPlan,
  moved,
  movedTo,
  normalizePlan,
  oneWorking,
  orderByDependencies,
  saveRunPlan,
  suggestedPlan,
  toStartRequest,
  usageOf,
  withUse,
} from './runPlan';

const project = (id: string, dependsOn: string[] = []) => ({
  id,
  name: id,
  path: `/${id}`,
  role: null,
  dependsOn,
});
// The frontend consumes the backend: the backend is worked in first.
const fullstack = { projects: [project('front', ['back']), project('back')] };
const single = { projects: [project('only')] };
const many = {
  projects: [project('a'), project('b'), project('c'), project('d')],
};

describe('run plan over several projects', () => {
  it('changes only the primary project by default and consults the others', () => {
    expect(suggestedPlan(fullstack)).toEqual({
      workingProjectIds: ['front'],
      readProjectIds: ['back'],
    });
    expect(suggestedPlan(many)).toEqual({
      workingProjectIds: ['a'],
      readProjectIds: ['b', 'c', 'd'],
    });
  });

  it('orders the projects the person marks by their dependencies', () => {
    expect(orderByDependencies(fullstack.projects).map((p) => p.id)).toEqual(['back', 'front']);
    expect(allWorking(fullstack)).toEqual({
      workingProjectIds: ['back', 'front'],
      readProjectIds: [],
    });
  });

  it('sends nothing for a one-project workspace, and the rest of the chain otherwise', () => {
    expect(toStartRequest(suggestedPlan(single), single)).toEqual({});
    expect(toStartRequest(allWorking(fullstack), fullstack)).toEqual({
      scope: { workingProjectId: 'back', readProjectIds: [] },
      nextProjectIds: ['front'],
    });
    expect(toStartRequest(oneWorking(fullstack, 'back'), fullstack)).toEqual({
      scope: { workingProjectId: 'back', readProjectIds: ['front'] },
    });
  });

  it('never leaves a run with nothing to change', () => {
    const plan = oneWorking(fullstack, 'back');

    expect(withUse(plan, 'back', 'read', fullstack)).toBe(plan);
    expect(withUse(plan, 'back', 'none', fullstack)).toBe(plan);
  });

  it('places a project newly changed after what it depends on, and a project is used one way', () => {
    const plan = withUse(oneWorking(fullstack, 'back'), 'front', 'work', fullstack);

    expect(plan).toEqual({ workingProjectIds: ['back', 'front'], readProjectIds: [] });
    expect(usageOf(plan, 'front')).toBe('work');
    const read = withUse(plan, 'front', 'read', fullstack);
    expect(read).toEqual({ workingProjectIds: ['back'], readProjectIds: ['front'] });
    expect(isChain(plan)).toBe(true);
    expect(isChain(read)).toBe(false);
  });

  it('lets the person reorder the runs', () => {
    const plan = allWorking(fullstack);

    expect(moved(plan, 'front', -1).workingProjectIds).toEqual(['front', 'back']);
    expect(moved(plan, 'back', -1)).toBe(plan);
  });

  it('moves a dragged project to where it was dropped', () => {
    const three = {
      projects: [project('a'), project('b'), project('c')],
    };
    const plan = allWorking(three);

    expect(movedTo(plan, 'c', 0).workingProjectIds).toEqual(['c', 'a', 'b']);
    expect(movedTo(plan, 'a', 2).workingProjectIds).toEqual(['b', 'c', 'a']);
    expect(movedTo(plan, 'a', 9).workingProjectIds).toEqual(['b', 'c', 'a']);
    expect(movedTo(plan, 'b', 1)).toBe(plan);
    expect(movedTo(plan, 'gone', 0)).toBe(plan);
  });

  it('drops removed projects and falls back to the suggestion when nothing is left to change', () => {
    expect(
      normalizePlan(
        { workingProjectIds: ['gone', 'front'], readProjectIds: ['front', 'back'] },
        fullstack,
      ),
    ).toEqual({ workingProjectIds: ['front'], readProjectIds: ['back'] });
    expect(normalizePlan({ workingProjectIds: ['gone'], readProjectIds: [] }, fullstack)).toEqual(
      suggestedPlan(fullstack),
    );
  });

  it('remembers the choice per workspace and survives garbage', () => {
    saveRunPlan('ws', { workingProjectIds: ['back'], readProjectIds: ['front'] });
    expect(loadRunPlan('ws')).toEqual({ workingProjectIds: ['back'], readProjectIds: ['front'] });
    localStorage.setItem('atlas.runPlan.bad', '{"workingProjectIds":3}');
    expect(loadRunPlan('bad')).toBeUndefined();
    expect(loadRunPlan('none')).toBeUndefined();
  });
});
