export interface TaskParts {
  /** What is to be done. */
  title: string;
  /** How it is known to be done, when the task says. */
  verification: string | null;
  /** The requirement of the specs it implements. */
  requirement: string | null;
  /** The tasks it waits for (`1.2`, `1.3`). */
  dependsOn: string[];
  /** An assumption the task states. */
  assumption: string | null;
}

/**
 * A task as the task list writes it: what to do, then `Verificação:`/`Verification:`, then maybe an
 * `Assumption:`, `depends on 1.2` and `(Requirement: name)`. Read here only to lay the line out;
 * a task that does not follow the shape is shown whole as its title.
 */
export function taskParts(text: string): TaskParts {
  let rest = text.trim();
  let requirement: string | null = null;
  const req = /\s*\((?:Requirement|Requisito):\s*([^)]+)\)\s*$/i.exec(rest);
  if (req) {
    requirement = req[1]?.trim() ?? null;
    rest = rest.slice(0, req.index).trim();
  }
  let dependsOn: string[] = [];
  const dep = /\s*(?:depends on|depende de)\s+([\d.]+(?:\s*(?:,|e|and)\s*[\d.]+)*)\.?\s*$/i.exec(
    rest,
  );
  if (dep) {
    dependsOn = (dep[1] ?? '').split(/\s*(?:,|e|and)\s*/i).filter((id) => id !== '');
    rest = rest.slice(0, dep.index).trim();
  }
  let assumption: string | null = null;
  const assume = /\s*(?:Assumption|Premissa):\s*(.*)$/is.exec(rest);
  if (assume) {
    assumption = assume[1]?.trim() ?? null;
    rest = rest.slice(0, assume.index).trim();
  }
  let verification: string | null = null;
  const verify = /\s*(?:Verificação|Verification):\s*(.*)$/is.exec(rest);
  if (verify) {
    verification = verify[1]?.trim() ?? null;
    rest = rest.slice(0, verify.index).trim();
  }
  return { title: rest, verification, requirement, dependsOn, assumption };
}

/** The group of a task from its number (`3.2` is group `3`). */
export function groupOf(id: string): string {
  return id.split('.')[0] ?? id;
}
