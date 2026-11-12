import { isAndroid } from './platform';

/**
 * The Android app draws edge-to-edge, so the status bar (top) and the navigation
 * bar (bottom) sit on top of the page. Newer WebViews report their size through
 * env(safe-area-inset-*); older ones report 0. When that happens we fall back to
 * typical bar heights so buttons never end up underneath the navigation bar.
 * (Runs once when the app starts; does nothing on Windows and Mac.)
 */
function applySafeAreaFallback() {
  if (!isAndroid) return;
  const probe = document.createElement('div');
  probe.style.cssText = 'position:fixed;left:0;top:0;visibility:hidden;pointer-events:none;'
    + 'padding-top:env(safe-area-inset-top,0px);padding-bottom:env(safe-area-inset-bottom,0px)';
  document.body.append(probe);
  const cs = getComputedStyle(probe);
  const top = parseFloat(cs.paddingTop) || 0;
  const bottom = parseFloat(cs.paddingBottom) || 0;
  probe.remove();
  const root = document.documentElement.style;
  if (top < 1) root.setProperty('--safe-t', '24px');
  if (bottom < 1) root.setProperty('--safe-b', '56px');
}

applySafeAreaFallback();
