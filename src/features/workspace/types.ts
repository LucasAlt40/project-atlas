import type {
  ExecutionEventDto,
  GridPositionDto,
  MessageDto,
  ProjectContextDto,
  SentMessageDto,
  StoredExecutionDto,
  WorkspaceDto,
  WorkspaceInputDto,
} from '@/lib/tauri/commands';

// The wire types are the domain types for now; aliased so features do not import DTOs by name.
export type Workspace = WorkspaceDto;
export type WorkspaceInput = WorkspaceInputDto;
export type ProjectContext = ProjectContextDto;
export type GridPosition = GridPositionDto;
export type Message = MessageDto;
export type SentMessage = SentMessageDto;
export type ExecutionEvent = ExecutionEventDto;
export type StoredExecution = StoredExecutionDto;
