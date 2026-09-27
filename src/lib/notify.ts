import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * Telling you a job has finished, when you are not already looking.
 *
 * A transcription runs for the better part of an hour, so the whole point is
 * that you can go and do something else. Notifying while the window is in
 * front would only be noise, so it does not.
 *
 * Permission is asked for at the moment there is something to say, not at
 * launch: a prompt before the app has done anything is a prompt you deny.
 */
export async function tell(title: string, body: string) {
  try {
    if (await getCurrentWindow().isFocused()) return;

    let allowed = await isPermissionGranted();
    if (!allowed) allowed = (await requestPermission()) === "granted";
    if (!allowed) return;

    sendNotification({ title, body });
  } catch {
    // Being told is a courtesy; never let it break the run that just finished.
  }
}

/** "3 lectures finished" -- what the notification says about a run. */
export function summarise(done: number, failed: number): string | null {
  if (done + failed === 0) return null;
  const lectures = `${done} ${done === 1 ? "lecture" : "lectures"}`;
  if (failed === 0) return `${lectures} finished.`;
  if (done === 0) return `${failed} ${failed === 1 ? "lecture" : "lectures"} failed.`;
  return `${lectures} finished, ${failed} failed.`;
}
