import { invokeCommand } from '@/lib/tauri/commands';
import type { AppInfo } from '../types/appInfo';

export async function fetchAppInfo(): Promise<AppInfo> {
  return invokeCommand('get_app_info');
}
