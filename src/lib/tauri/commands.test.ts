import { invoke } from '@tauri-apps/api/core';
import { CommandError, invokeCommand } from './commands';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('invokeCommand', () => {
  it('forwards the command name and returns the typed result', async () => {
    const info = { name: 'Project Atlas', version: '1.0.0', platform: 'linux' };
    vi.mocked(invoke).mockResolvedValueOnce(info);

    await expect(invokeCommand('get_app_info')).resolves.toEqual(info);
    expect(invoke).toHaveBeenCalledWith('get_app_info', undefined);
  });

  it('wraps failures in CommandError', async () => {
    vi.mocked(invoke).mockRejectedValueOnce('boom');

    await expect(invokeCommand('get_app_info')).rejects.toBeInstanceOf(CommandError);
  });

  it('passes the send_message request under the `request` argument', async () => {
    const request = { workspaceId: 'w', agentId: 'a', content: 'hi' };
    vi.mocked(invoke).mockResolvedValueOnce({});

    await invokeCommand('send_message', { request });

    expect(invoke).toHaveBeenCalledWith('send_message', { request });
  });

  it('keeps the core’s structured error so the UI can word it', async () => {
    const appError = { code: 'project_folder_not_found', params: { path: '/x' }, detail: null };
    vi.mocked(invoke).mockRejectedValueOnce(appError);

    const error = await invokeCommand('get_settings').catch((e: unknown) => e);

    expect(error).toBeInstanceOf(CommandError);
    expect((error as CommandError).appError).toEqual(appError);
  });
});
