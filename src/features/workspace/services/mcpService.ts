import { invokeCommand } from '@/lib/tauri/commands';
import type {
  McpConnectionDto,
  McpGrantDto,
  McpOverviewDto,
  McpPresetDto,
  McpToolSelectionDto,
  McpTransportDto,
} from '@/lib/tauri/commands';

export function listMcpConnections(workspaceId: string): Promise<McpOverviewDto> {
  return invokeCommand('list_mcp_connections', { workspaceId });
}

export function listMcpCatalog(): Promise<McpPresetDto[]> {
  return invokeCommand('list_mcp_catalog');
}

export function addMcpPreset(workspaceId: string, presetId: string): Promise<McpConnectionDto> {
  return invokeCommand('add_mcp_preset', { workspaceId, presetId });
}

export function addMcpConnection(
  workspaceId: string,
  name: string,
  transport: McpTransportDto,
  required: boolean,
): Promise<McpConnectionDto> {
  return invokeCommand('add_mcp_connection', { workspaceId, name, transport, required });
}

export function setMcpConnectionEnabled(
  connectionId: string,
  enabled: boolean,
): Promise<McpConnectionDto> {
  return invokeCommand('set_mcp_connection_enabled', { connectionId, enabled });
}

export function removeMcpConnection(connectionId: string): Promise<undefined> {
  return invokeCommand('remove_mcp_connection', { connectionId });
}

/** The value goes to the OS credential store; nothing of it comes back. */
export function setMcpSecret(
  connectionId: string,
  name: string,
  value: string,
): Promise<McpConnectionDto> {
  return invokeCommand('set_mcp_secret', { connectionId, name, value });
}

export function clearMcpSecret(connectionId: string, name: string): Promise<McpConnectionDto> {
  return invokeCommand('clear_mcp_secret', { connectionId, name });
}

export function probeMcpConnection(
  connectionId: string,
  runtimeId: string,
): Promise<McpConnectionDto> {
  return invokeCommand('probe_mcp_connection', { connectionId, runtimeId });
}

export function grantMcpConnection(
  connectionId: string,
  agentId: string,
  tools: McpToolSelectionDto,
): Promise<McpGrantDto> {
  return invokeCommand('grant_mcp_connection', { connectionId, agentId, tools });
}

export function revokeMcpGrant(grantId: string): Promise<undefined> {
  return invokeCommand('revoke_mcp_grant', { grantId });
}
