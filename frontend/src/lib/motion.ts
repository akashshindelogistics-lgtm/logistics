import { useEffect, useRef, useState } from 'react';
import { animate } from 'motion/react';
import autoAnimate from '@formkit/auto-animate';
import { useAutoAnimate } from '@formkit/auto-animate/react';

// Shared motion settings. Everything that moves goes through here or through
// motion/react (which reads the same OS setting via <MotionConfig
// reducedMotion="user">), so "reduce motion" switches all of it off at once.

export const REDUCED_MOTION_QUERY = '(prefers-reduced-motion: reduce)';

/** Durations in seconds (motion/react) — keep UI motion within 150–300 ms. */
export const DURATION = { fast: 0.15, base: 0.22, slow: 0.3 } as const;

/** A soft ease-out used for entrances. */
export const EASE_OUT = [0.22, 1, 0.36, 1] as const;

/** Whether the user has asked the OS for reduced motion, right now. */
export function prefersReducedMotion(): boolean {
  return typeof matchMedia === 'function' && matchMedia(REDUCED_MOTION_QUERY).matches;
}

/** `prefersReducedMotion()` as state that follows the OS setting live. */
export function useReducedMotion(): boolean {
  const [reduced, setReduced] = useState(prefersReducedMotion);
  useEffect(() => {
    if (typeof matchMedia !== 'function') return;
    const query = matchMedia(REDUCED_MOTION_QUERY);
    const onChange = () => setReduced(query.matches);
    query.addEventListener('change', onChange);
    return () => query.removeEventListener('change', onChange);
  }, []);
  return reduced;
}

/**
 * Animate a list container's children as they're added, removed or
 * reordered — put the returned ref on a `<tbody>` or `<ul>`. auto-animate
 * already does nothing when reduced motion is on.
 */
export function useListAnimation<T extends HTMLElement = HTMLTableSectionElement>() {
  const [ref] = useAutoAnimate<T>({ duration: 220, easing: 'ease-out' });
  return ref;
}

/**
 * A number that counts up (or down) to `target` whenever it changes, for
 * stat tiles. Jumps straight there when reduced motion is on.
 */
export function useCountUp(target: number, duration = 0.7): number {
  const [shown, setShown] = useState(0);
  const current = useRef(0);
  useEffect(() => {
    if (prefersReducedMotion()) {
      current.current = target;
      setShown(target);
      return;
    }
    const controls = animate(current.current, target, {
      duration,
      ease: EASE_OUT,
      onUpdate: v => {
        current.current = v;
        setShown(Math.round(v));
      },
    });
    return () => controls.stop();
  }, [target, duration]);
  return shown;
}

/** How often maps showing vehicles re-fetch positions, in ms. */
export const LIVE_POLL_MS = 10_000;

/**
 * Call `refresh` every `intervalMs` while the tab is visible (and once on
 * returning to it), so live maps pick up new GPS fixes and their markers can
 * glide to them. The latest `refresh` is always the one called.
 */
export function useLivePolling(refresh: () => void, intervalMs = LIVE_POLL_MS) {
  const latest = useRef(refresh);
  useEffect(() => { latest.current = refresh; });
  useEffect(() => {
    const tick = () => { if (document.visibilityState === 'visible') latest.current(); };
    const id = window.setInterval(tick, intervalMs);
    document.addEventListener('visibilitychange', tick);
    return () => {
      window.clearInterval(id);
      document.removeEventListener('visibilitychange', tick);
    };
  }, [intervalMs]);
}

/**
 * Ref callback version of useListAnimation, for list containers rendered
 * inside a loop (where a hook cannot be called). Stable across renders, and
 * marks the element so it is only set up once.
 */
export function animateList(el: HTMLElement | null) {
  if (!el || el.dataset.listAnimated) return;
  el.dataset.listAnimated = '1';
  autoAnimate(el, { duration: 220, easing: 'ease-out' });
}
