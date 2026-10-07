import { invoke } from '@tauri-apps/api/core';
import type { AppErrorDto, CommandMap, CommandName } from './dto';

// The wire types live in `dto.ts`; this file is the runtime side of the same contract.
export * from './dto';

export function isAppError(value: unknown): value is AppErrorDto {
  return (
    typeof value === 'object' &&
    value !== null &&
    'code' in value &&
    typeof value.code === 'string' &&
    'params' in value
  );
}

export class CommandError extends Error {
  /** The core's structured error, when it sent one. */
  readonly appError: AppErrorDto | undefined;

  constructor(
    readonly command: CommandName,
    cause: unknown,
  ) {
    super(
      `Command "${command}" failed: ${
        isAppError(cause) ? cause.code : cause instanceof Error ? cause.message : String(cause)
      }`,
      { cause },
    );
    this.name = 'CommandError';
    this.appError = isAppError(cause) ? cause : undefined;
  }
}

export async function invokeCommand<C extends CommandName>(
  command: C,
  ...args: CommandMap[C]['args'] extends undefined ? [] : [CommandMap[C]['args']]
): Promise<CommandMap[C]['result']> {
  try {
    return await invoke<CommandMap[C]['result']>(command, args[0]);
  } catch (error) {
    throw new CommandError(command, error);
  }
}
