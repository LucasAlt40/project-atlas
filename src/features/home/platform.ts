/** The operating system's own name for the platform id the core reports (`macos` → `macOS`). */
export function platformName(platform: string): string {
  const names: Record<string, string> = { macos: 'macOS', windows: 'Windows', linux: 'Linux' };
  return names[platform.toLowerCase()] ?? platform;
}
