/** Runtime platform checks. The Android app runs the same pages in Android's WebView. */
import { copyText } from './clipboard';
import { toast } from './dom';

export const isAndroid = /Android/i.test(navigator.userAgent);
export const isMobile = isAndroid || /iPhone|iPad/i.test(navigator.userAgent);
export const RELEASES_URL = 'https://github.com/treasure155/farmbuddy/releases/latest';

/**
 * Opens a link in the phone's or computer's own apps (WhatsApp, email, browser).
 * The app window itself is never sent to the link: on Android that replaced the app
 * with a "Webpage not available" page.
 */
export async function openExternal(url: string): Promise<boolean> {
  try {
    const { openUrl } = await import('@tauri-apps/plugin-opener');
    await openUrl(url);
    return true;
  } catch {
    if (!isMobile && !url.startsWith('mailto:')) {
      const w = window.open(url, '_blank', 'noopener');
      if (w) return true;
    }
    const copied = await copyText(url.startsWith('mailto:') ? url.slice(7).split('?')[0] : url);
    toast(copied ? 'Could not open it here, so the link was copied. Paste it in your browser or email app.' : 'Could not open the link on this device.', 'error');
    return false;
  }
}
