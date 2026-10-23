import type { ConfigNoticeDto } from '@/lib/tauri/commands';

export interface AppInfo {
  name: string;
  version: string;
  platform: string;
  configNotice?: ConfigNoticeDto;
}
