/** Copies text to the clipboard. Works in the Android app too, where navigator.clipboard is often blocked. */
export async function copyText(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard?.writeText) { await navigator.clipboard.writeText(text); return true; }
  } catch { /* fall through to the older method */ }
  try {
    const box = document.createElement('textarea');
    box.value = text;
    box.setAttribute('readonly', '');
    box.style.cssText = 'position:fixed;top:0;left:0;width:1px;height:1px;opacity:0;font-size:16px';
    document.body.append(box);
    box.focus();
    box.select();
    box.setSelectionRange(0, text.length);
    const ok = document.execCommand('copy');
    box.remove();
    return ok;
  } catch { return false; }
}
