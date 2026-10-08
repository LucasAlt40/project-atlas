import { invokeCommand, type UpdateInfoDto } from '@/lib/tauri/commands';

/** `null` when this is already the newest version. */
export async function checkForUpdate(): Promise<UpdateInfoDto | null> {
  return invokeCommand('check_for_update');
}

/** Downloads and installs the newest version; the app restarts when it is done. */
export async function installUpdate(): Promise<void> {
  await invokeCommand('install_update');
}
