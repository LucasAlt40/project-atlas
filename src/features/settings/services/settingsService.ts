import {
  invokeCommand,
  type AppSettingsDto,
  type ExternalCheckDto,
  type ExternalMemorySettingsDto,
  type ExternalSyncDto,
  type MemorySettingsDto,
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

/** The memory server on this machine that notes may be read from (off by default; loopback only). */
export function setExternalMemory(external: ExternalMemorySettingsDto): Promise<AppSettingsDto> {
  return invokeCommand('set_external_memory', { external });
}

/** Whether the memory server answers with Atlas's token, using the saved settings. */
export function checkExternalMemory(): Promise<ExternalCheckDto> {
  return invokeCommand('check_external_memory');
}

/** Publishes the approved notes to the memory server and retires the ones out of use. */
export function syncExternalMemory(): Promise<ExternalSyncDto> {
  return invokeCommand('sync_external_memory');
}

/** The most tokens the runs of one specification may use together (`null`: no limit). */
export function setSpecsMaxTokens(max: number | null): Promise<AppSettingsDto> {
  return invokeCommand('set_specs_max_tokens', { max });
}

/** What Atlas shares on the person's Discord profile (Rich Presence; off by default). */
export function setPresenceSettings(presence: PresenceSettingsDto): Promise<AppSettingsDto> {
  return invokeCommand('set_presence_settings', { presence });
}
