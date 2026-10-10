import { afterEach, describe, it, expect, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import { prefersReducedMotion, useCountUp, useLivePolling, useReducedMotion } from './motion';

const realMatchMedia = window.matchMedia;
const withMotion = (reduce: boolean) => {
  window.matchMedia = ((query: string) => ({
    ...realMatchMedia(query),
    matches: reduce && query.includes('reduce'),
  })) as typeof window.matchMedia;
};

describe('motion helpers', () => {
  afterEach(() => {
    window.matchMedia = realMatchMedia;
    vi.useRealTimers();
  });

  it('reads the OS reduced-motion setting (the test setup reports it on)', () => {
    expect(prefersReducedMotion()).toBe(true);
    withMotion(false);
    expect(prefersReducedMotion()).toBe(false);
    const { result } = renderHook(() => useReducedMotion());
    expect(result.current).toBe(false);
  });

  it('useCountUp jumps straight to the value under reduced motion', () => {
    const { result, rerender } = renderHook(({ v }) => useCountUp(v), { initialProps: { v: 42 } });
    expect(result.current).toBe(42);
    rerender({ v: 7 });
    expect(result.current).toBe(7);
  });

  it('useCountUp animates from where it is when motion is allowed', async () => {
    withMotion(false);
    const { result } = renderHook(() => useCountUp(100, 0.05));
    expect(result.current).toBe(0);
    await vi.waitFor(() => expect(result.current).toBe(100), { timeout: 2000 });
  });

  it('useLivePolling calls the latest refresh on each interval while the tab is visible', () => {
    vi.useFakeTimers();
    const first = vi.fn();
    const second = vi.fn();
    const { rerender, unmount } = renderHook(({ fn }) => useLivePolling(fn, 1000), { initialProps: { fn: first } });
    act(() => { vi.advanceTimersByTime(1000); });
    expect(first).toHaveBeenCalledTimes(1);

    rerender({ fn: second });
    act(() => { vi.advanceTimersByTime(1000); });
    expect(second).toHaveBeenCalledTimes(1);
    expect(first).toHaveBeenCalledTimes(1);

    unmount();
    act(() => { vi.advanceTimersByTime(5000); });
    expect(second).toHaveBeenCalledTimes(1);
  });

  it('useLivePolling skips refreshes while the tab is hidden', () => {
    vi.useFakeTimers();
    const fn = vi.fn();
    const visibility = vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('hidden');
    renderHook(() => useLivePolling(fn, 1000));
    act(() => { vi.advanceTimersByTime(3000); });
    expect(fn).not.toHaveBeenCalled();
    visibility.mockReturnValue('visible');
    act(() => { document.dispatchEvent(new Event('visibilitychange')); });
    expect(fn).toHaveBeenCalledTimes(1);
    visibility.mockRestore();
  });
});
