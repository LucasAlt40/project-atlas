/** The person's last choice about approving plans automatically, kept on this machine. */
const KEY = 'atlas.autoApprovePlans';

export function loadAutoPlans(): boolean {
  try {
    return localStorage.getItem(KEY) === 'true';
  } catch {
    return false;
  }
}

export function saveAutoPlans(value: boolean): void {
  try {
    localStorage.setItem(KEY, String(value));
  } catch {
    // Not kept: the choice still holds for this screen.
  }
}
