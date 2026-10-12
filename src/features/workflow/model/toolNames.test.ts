import { readsOnly } from './toolNames';

describe('which tools only read', () => {
  it('tells a reading tool from a writing one by the first word of its name', () => {
    for (const name of [
      'get_issue',
      'getIssue',
      'search-issues',
      'list_pages',
      'resolve-library-id',
    ]) {
      expect(readsOnly(name), name).toBe(true);
    }
    for (const name of [
      'create_issue',
      'updateIssue',
      'delete_page',
      'navigate_page',
      'getaway_x',
    ]) {
      expect(readsOnly(name), name).toBe(false);
    }
  });
});
