import {
  invokeCommand,
  type AppSettingsDto,
  type MemorySettingsDto,
  type PresenceScreenDto,
  type PresenceSettingsDto,
  type SandboxSettingsDto,
  type SandboxStatusDto,
} from '@/lib/tauri/commands';

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

/** What this system can confine, and the person's choice (ADR 0038). */
export function getSandboxStatus(): Promise<SandboxStatusDto> {
  return invokeCommand('get_sandbox_status');
}

export function setSandboxSettings(sandbox: SandboxSettingsDto): Promise<AppSettingsDto> {
  return invokeCommand('set_sandbox_settings', { sandbox });
}

/** Which scopes of the Memory are recalled and proposed to, and the most a prompt may spend. */
export function setMemorySettings(memory: MemorySettingsDto): Promise<AppSettingsDto> {
  return invokeCommand('set_memory_settings', { memory });
}

/** The most tokens the runs of one specification may use together (`null`: no limit). */
export function setSpecsMaxTokens(max: number | null): Promise<AppSettingsDto> {
  return invokeCommand('set_specs_max_tokens', { max });
}

/** What Atlas shares on the person's Discord profile (Rich Presence; off by default). */
export function setPresenceSettings(presence: PresenceSettingsDto): Promise<AppSettingsDto> {
  return invokeCommand('set_presence_settings', { presence });
}

/** Tells the core which screen is open; it picks a line of the Discord profile when idle. */
export function setPresenceScreen(screen: PresenceScreenDto): Promise<null> {
  return invokeCommand('set_presence_screen', { screen });
}
