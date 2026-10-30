import type { MouseEvent } from 'react';
import { flushSync } from 'react-dom';
import { useNavigate, type To } from 'react-router-dom';
import { prefersReducedMotion } from './motion';

// Page transitions with the browser's View Transitions API. React Router's own
// `viewTransition` prop needs a data router (RouterProvider); this app uses
// <BrowserRouter>, so navigation is wrapped here instead. Browsers without the
// API, and users who prefer reduced motion, just navigate as before.

/** The `view-transition-name` shared by a vehicle's row icon and its detail-page header icon. */
export const VEHICLE_HERO = 'vehicle-hero';

export function canViewTransition(): boolean {
  return typeof document !== 'undefined'
    && typeof document.startViewTransition === 'function'
    && !prefersReducedMotion();
}

interface Options {
  state?: unknown;
  replace?: boolean;
  /**
   * Gives the clicked element a `view-transition-name` for the duration of
   * the transition, so it morphs into the element on the next page that
   * carries the same name.
   */
  morph?: { name: string; element: () => Element | null | undefined };
}

/**
 * An onClick for a <Link>/<NavLink> that performs the navigation inside a
 * view transition. Modified clicks (new tab, etc.) are left to the browser.
 */
export function useTransitionClick(to: To, { state, replace, morph }: Options = {}) {
  const navigate = useNavigate();
  return (e: MouseEvent<HTMLAnchorElement>) => {
    if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    if (!canViewTransition()) return;
    e.preventDefault();
    const el = morph?.element() as HTMLElement | null | undefined;
    if (el) el.style.viewTransitionName = morph!.name;
    const transition = document.startViewTransition(() => {
      flushSync(() => navigate(to, { state, replace }));
    });
    transition.finished.finally(() => {
      if (el) el.style.viewTransitionName = '';
    });
  };
}
