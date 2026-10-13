import { groupOf, taskParts } from './taskText';

describe('taskParts', () => {
  it('lays out a task the way the task list writes it', () => {
    const parts = taskParts(
      'Criar o repositório. Verificação: teste com 0, 1 e 2 linhas. Assumption: o script fica em `docs/`. depends on 1.1, 1.2 (Requirement: Exclusão da própria conta)',
    );

    expect(parts).toEqual({
      title: 'Criar o repositório.',
      verification: 'teste com 0, 1 e 2 linhas.',
      assumption: 'o script fica em `docs/`.',
      dependsOn: ['1.1', '1.2'],
      requirement: 'Exclusão da própria conta',
    });
  });

  it('reads English markers and a task with only part of the shape', () => {
    expect(
      taskParts('Write the module. Verification: a test passes. depends on 2.1'),
    ).toMatchObject({
      title: 'Write the module.',
      verification: 'a test passes.',
      dependsOn: ['2.1'],
      requirement: null,
    });
  });

  it('shows a task that follows no shape whole', () => {
    expect(taskParts('Just do the thing')).toEqual({
      title: 'Just do the thing',
      verification: null,
      requirement: null,
      dependsOn: [],
      assumption: null,
    });
  });
});

describe('groupOf', () => {
  it('is the number before the first dot', () => {
    expect(groupOf('3.2')).toBe('3');
    expect(groupOf('7')).toBe('7');
  });
});
