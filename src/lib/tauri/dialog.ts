import { open } from '@tauri-apps/plugin-dialog';

/**
 * Opens the operating system's folder picker and returns the chosen folder, or `null` if the
 * user cancelled. The core re-checks that the path is a real folder; the picker only chooses.
 */
export async function pickFolder(defaultPath?: string): Promise<string | null> {
  const chosen = await open({
    directory: true,
    multiple: false,
    ...(defaultPath ? { defaultPath } : {}),
  });
  return typeof chosen === 'string' ? chosen : null;
}
