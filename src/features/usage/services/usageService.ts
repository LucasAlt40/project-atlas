import { invokeCommand } from '@/lib/tauri/commands';
import type { AgentUsageSummaryDto, WorkspaceUsageSummaryDto } from '@/lib/tauri/commands';
import { usageWindows } from '../model/windows';

/** What Atlas observed agents consume. Sums are over executions Atlas saw, nothing more. */

export function getAgentUsage(workspaceId: string, agentId: string): Promise<AgentUsageSummaryDto> {
  return invokeCommand('get_agent_usage', { workspaceId, agentId, windows: usageWindows() });
}

export function getWorkspaceUsage(workspaceId: string): Promise<WorkspaceUsageSummaryDto> {
  return invokeCommand('get_workspace_usage', { workspaceId, windows: usageWindows() });
}
