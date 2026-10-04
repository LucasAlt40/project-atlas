import { sendNotification } from '@tauri-apps/plugin-notification';

/**
 * A system notification (with the system's default sound). Never throws: a notification is a
 * courtesy, not something a workflow can depend on.
 *
 * The permission helpers of the plugin are deliberately not used. On desktop the core always
 * reports the permission as granted (the operating system asks the person itself, the first
 * time a notification is shown), but those helpers look at the webview's own `Notification`
 * API first, which is "denied" in the macOS webview: asking it would suppress every
 * notification before the system was ever asked.
 */
export function notifyUser(title: string, body: string): Promise<boolean> {
  try {
    sendNotification({ title, body, sound: 'default' });
    return Promise.resolve(true);
  } catch (error) {
    console.warn('Could not show a system notification', error);
    return Promise.resolve(false);
  }
}

let audio: AudioContext | undefined;

/**
 * A short two-note chime made in the webview itself. It does not depend on the operating
 * system's notification settings (which can silently drop a notification, as in a development
 * build), so the person is always told by sound while Atlas is running.
 */
export function chime(): void {
  try {
    audio ??= new AudioContext();
    const context = audio;
    void context.resume();
    const start = context.currentTime;
    for (const [index, frequency] of [880, 1175].entries()) {
      const oscillator = context.createOscillator();
      const gain = context.createGain();
      oscillator.frequency.value = frequency;
      gain.gain.setValueAtTime(0.0001, start + index * 0.18);
      gain.gain.exponentialRampToValueAtTime(0.25, start + index * 0.18 + 0.02);
      gain.gain.exponentialRampToValueAtTime(0.0001, start + index * 0.18 + 0.3);
      oscillator.connect(gain).connect(context.destination);
      oscillator.start(start + index * 0.18);
      oscillator.stop(start + index * 0.18 + 0.32);
    }
  } catch {
    // No audio available: the notification and the indicator remain.
  }
}
