import { invokeCommand, type AppSettingsDto } from '@/lib/tauri/commands';

/** Settings of the whole application (not of any workspace), stored by the core. */

export function getSettings(): Promise<AppSettingsDto> {
  return invokeCommand('get_settings');
}

export function setLanguage(language: string): Promise<AppSettingsDto> {
  return invokeCommand('set_language', { language });
}

/** Remembers which workspace is open, so it opens again next time (`null`: none). */
export function selectWorkspace(workspaceId: string | null): Promise<AppSettingsDto> {
  return invokeCommand('select_workspace', { workspaceId });
}

/** The most tokens the runs of one specification may use together (`null`: no limit). */
export function setSpecsMaxTokens(max: number | null): Promise<AppSettingsDto> {
  return invokeCommand('set_specs_max_tokens', { max });
}
