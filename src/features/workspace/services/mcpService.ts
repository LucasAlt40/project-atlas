import { invokeCommand } from '@/lib/tauri/commands';
import type {
  McpApplyReportDto,
  McpConfigDialectDto,
  McpConnectionDto,
  McpImportReportDto,
  McpRegistryPageDto,
  McpRuntimeSourceDto,
  McpSyncPlanDto,
  McpSyncReportDto,
  McpGrantDto,
  McpOverviewDto,
  McpPresetDto,
  McpToolSelectionDto,
  McpTransportDto,
  WorkflowDto,
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

/** The workflows of the workspace, to say which run or step a grant is for. */
export function listWorkspaceWorkflows(workspaceId: string): Promise<WorkflowDto[]> {
  return invokeCommand('list_workflows', { workspaceId });
}

export function grantMcpConnection(
  connectionId: string,
  agentId: string,
  tools: McpToolSelectionDto,
  scope: { workflowId?: string; nodeId?: string } = {},
): Promise<McpGrantDto> {
  return invokeCommand('grant_mcp_connection', { connectionId, agentId, tools, ...scope });
}

export function revokeMcpGrant(grantId: string): Promise<undefined> {
  return invokeCommand('revoke_mcp_grant', { grantId });
}

/** Changes how a connection starts. It is switched off again and what was learned is forgotten. */
export function updateMcpConnection(
  connectionId: string,
  transport: McpTransportDto,
  required: boolean,
): Promise<McpConnectionDto> {
  return invokeCommand('update_mcp_connection', { connectionId, transport, required });
}

/** Adds what a configuration text holds as connections that are off and granted to nobody. */
export function importMcpConfig(
  workspaceId: string,
  text: string,
  dialect?: McpConfigDialectDto,
): Promise<McpImportReportDto> {
  return invokeCommand('import_mcp_config', {
    workspaceId,
    text,
    ...(dialect !== undefined && { dialect }),
  });
}

/** The connections (all, or the ones named) as a CLI's configuration. No secret value is in it. */
export function exportMcpConfig(
  workspaceId: string,
  dialect: McpConfigDialectDto,
  connectionIds: string[] = [],
): Promise<string> {
  return invokeCommand('export_mcp_config', { workspaceId, dialect, connectionIds });
}

export function readRuntimeMcpConfig(
  workspaceId: string,
  dialect: McpConfigDialectDto,
): Promise<McpRuntimeSourceDto> {
  return invokeCommand('read_runtime_mcp_config', { workspaceId, dialect });
}

/** Adds connections to the CLI's own file; what is already there is left as it is. */
export function applyMcpToRuntime(
  workspaceId: string,
  dialect: McpConfigDialectDto,
  connectionIds: string[] = [],
): Promise<McpApplyReportDto> {
  return invokeCommand('apply_mcp_to_runtime', { workspaceId, dialect, connectionIds });
}

/** What each CLI's files hold, and which servers some CLI lacks. Reads, writes nothing. */
export function planMcpSync(workspaceId: string): Promise<McpSyncPlanDto> {
  return invokeCommand('plan_mcp_sync', { workspaceId });
}

/** Imports what the CLIs have and writes what each lacks to its file (nothing overwritten). */
export function applyMcpSync(workspaceId: string): Promise<McpSyncReportDto> {
  return invokeCommand('apply_mcp_sync', { workspaceId });
}

/** The official MCP Registry. An empty query lists; `cursor` is the previous page's. */
export function searchMcpRegistry(query: string, cursor?: string): Promise<McpRegistryPageDto> {
  return invokeCommand('search_mcp_registry', { query, ...(cursor !== undefined && { cursor }) });
}
