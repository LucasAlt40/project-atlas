import type {
  AgentDto,
  CreateAgentRequestDto,
  CreatePersonalityRequestDto,
  ModelDto,
  PersonalityDto,
  RuntimeStatusDto,
} from '@/lib/tauri/commands';

// The wire types are the domain types for now; aliased so features do not import DTOs by name.
export type Agent = AgentDto;
export type Personality = PersonalityDto;
export type RuntimeStatus = RuntimeStatusDto;
export type Model = ModelDto;
export type CreateAgentInput = CreateAgentRequestDto;
export type CreatePersonalityInput = CreatePersonalityRequestDto;
