import {
  consultsAll,
  defaultChoice,
  loadScopes,
  normalizeChoice,
  saveScopes,
  toScopeRequest,
  withRead,
  withWorking,
} from './scope';

const project = (id: string) => ({ id, name: id, path: `/${id}`, role: null, dependsOn: [] });
const shop = { projects: [project('front'), project('back'), project('orders')] };
const single = { projects: [project('only')] };

describe('execution scope', () => {
  it('is the primary project alone by default, and a workspace with one project sends nothing', () => {
    expect(defaultChoice(shop)).toEqual({ workingProjectId: 'front', readProjectIds: [] });
    expect(toScopeRequest(undefined, shop)).toBeUndefined();
    expect(toScopeRequest(defaultChoice(single), single)).toBeUndefined();
  });

  it('sends the working project and the ones consulted, in the workspace order', () => {
    const choice = { workingProjectId: 'back', readProjectIds: ['orders', 'front'] };

    expect(toScopeRequest(choice, shop)).toEqual({
      workingProjectId: 'back',
      readProjectIds: ['front', 'orders'],
    });
  });

  it('sends a different working project even when it consults nothing', () => {
    expect(toScopeRequest({ workingProjectId: 'orders', readProjectIds: [] }, shop)).toEqual({
      workingProjectId: 'orders',
      readProjectIds: [],
    });
  });

  it('drops what the workspace no longer has and never consults the working project', () => {
    const stale = { workingProjectId: 'gone', readProjectIds: ['back', 'gone', 'front'] };

    expect(normalizeChoice(stale, shop)).toEqual({
      workingProjectId: 'front',
      readProjectIds: ['back'],
    });
    expect(toScopeRequest(stale, shop)).toEqual({
      workingProjectId: 'front',
      readProjectIds: ['back'],
    });
  });

  it('moves the working project out of the consulted ones and back', () => {
    const choice = { workingProjectId: 'front', readProjectIds: ['back', 'orders'] };

    const moved = withWorking(choice, 'back');

    expect(moved).toEqual({ workingProjectId: 'back', readProjectIds: ['orders'] });
    expect(withRead(moved, 'back', true)).toEqual(moved);
    expect(withRead(moved, 'front', true).readProjectIds).toEqual(['orders', 'front']);
    expect(withRead(moved, 'orders', false).readProjectIds).toEqual([]);
  });

  it('can consult everything else at once', () => {
    expect(consultsAll(defaultChoice(shop), shop).readProjectIds).toEqual(['back', 'orders']);
  });

  it('remembers the choices of a workspace, and survives storage that is unreadable', () => {
    saveScopes('w1', { a1: { workingProjectId: 'back', readProjectIds: ['front'] } });

    expect(loadScopes('w1')).toEqual({
      a1: { workingProjectId: 'back', readProjectIds: ['front'] },
    });
    expect(loadScopes('other')).toEqual({});
    localStorage.setItem('atlas.scope.broken', '{nope');
    expect(loadScopes('broken')).toEqual({});
    localStorage.setItem('atlas.scope.odd', JSON.stringify({ a: 1, b: { workingProjectId: 2 } }));
    expect(loadScopes('odd')).toEqual({});
  });
});
