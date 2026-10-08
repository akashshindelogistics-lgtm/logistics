import { afterEach, describe, it, expect, vi } from 'vitest';
import { DISABLE_3D_KEY, can3D } from './webgl';

describe('can3D', () => {
  afterEach(() => {
    localStorage.clear();
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('is false without WebGL (as in jsdom)', () => {
    expect(can3D()).toBe(false);
  });

  it('is true when the browser can create a WebGL context', () => {
    vi.stubGlobal('WebGLRenderingContext', function WebGLRenderingContext() {});
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({} as never);
    expect(can3D()).toBe(true);
  });

  it('is false when 3D has been switched off, even with WebGL', () => {
    vi.stubGlobal('WebGLRenderingContext', function WebGLRenderingContext() {});
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({} as never);
    localStorage.setItem(DISABLE_3D_KEY, '1');
    expect(can3D()).toBe(false);
  });
});
