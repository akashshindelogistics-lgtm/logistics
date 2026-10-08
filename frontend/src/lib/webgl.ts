/**
 * localStorage key that forces the static poster instead of the live 3D view.
 * The headless Playwright suite sets it so screenshots and timings don't
 * depend on software WebGL; anyone on a slow machine can set it too.
 */
export const DISABLE_3D_KEY = 'logitrack:disable-3d';

/** Whether to render live WebGL scenes in this browser. */
export function can3D(): boolean {
  try {
    if (localStorage.getItem(DISABLE_3D_KEY) === '1') return false;
  } catch {
    // Storage blocked: carry on and decide from WebGL support alone.
  }
  if (typeof WebGLRenderingContext === 'undefined') return false;
  try {
    const canvas = document.createElement('canvas');
    return !!(canvas.getContext('webgl2') || canvas.getContext('webgl'));
  } catch {
    return false;
  }
}
