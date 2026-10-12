import { invokeCommand } from '@/lib/tauri/commands';
import type { RemoteStatusDto } from '@/lib/tauri/commands';

/**
 * The UI's only door to mobile access (ADR 0034). The core decides which address it may listen
 * on (one the computer has) and whether anything listens at all; the UI only asks.
 */

export function getRemoteStatus(): Promise<RemoteStatusDto> {
  return invokeCommand('get_remote_status');
}

/** Turns the phone server on (at `address`, or the recommended one) or off. */
export function setRemoteEnabled(
  enabled: boolean,
  address: string | null = null,
): Promise<RemoteStatusDto> {
  return invokeCommand('set_remote_enabled', { enabled, address });
}

export function newPairingCode(): Promise<RemoteStatusDto> {
  return invokeCommand('new_remote_pairing_code');
}

export function revokeDevice(deviceId: string): Promise<RemoteStatusDto> {
  return invokeCommand('revoke_remote_device', { deviceId });
}

export function revokeAllDevices(): Promise<RemoteStatusDto> {
  return invokeCommand('revoke_all_remote_devices');
}
