import type { DispatchOrder, DispatchStatus } from '../types';

export const STATUS_TAG_CLASS: Record<DispatchStatus, string> = {
  AWAITING_VEHICLE: 'tag-amber',
  PENDING: 'tag-amber',
  CONFIRMED: 'tag-blue',
  LOADED: 'tag-blue',
  IN_TRANSIT: 'tag-purple',
  DELIVERED: 'tag-green',
  RETURNED: 'tag-red',
  CANCELLED: 'tag-red',
};

export interface NextAction {
  status: DispatchStatus;
  label: string;
  variant: 'primary' | 'danger';
  requiresProof?: boolean;
  /** RETURNED credits the shipment's stock back into a godown; the UI lets
   *  the user pick which one (optional — the server has a sensible default). */
  isReturn?: boolean;
}

// Mirrors DispatchStatus::can_transition_to in src/logistics/dispatch/dispatch.rs
// — only the moves the backend actually allows are offered here. Keep in
// sync if the backend state machine changes; the backend is still the one
// that enforces this (a stale frontend map only means a wrong/missing
// button, never an illegal transition actually going through).
// AWAITING_VEHICLE -> PENDING isn't a status change: it happens when the
// hired truck is assigned (the Dispatches page's "Assign hired vehicle" form).
export const NEXT_ACTIONS: Partial<Record<DispatchStatus, NextAction[]>> = {
  AWAITING_VEHICLE: [
    { status: 'CANCELLED', label: 'Cancel', variant: 'danger' },
  ],
  PENDING: [
    { status: 'CONFIRMED', label: 'Confirm', variant: 'primary' },
    { status: 'CANCELLED', label: 'Cancel', variant: 'danger' },
  ],
  CONFIRMED: [
    { status: 'LOADED', label: 'Mark Loaded', variant: 'primary' },
    { status: 'CANCELLED', label: 'Cancel', variant: 'danger' },
  ],
  LOADED: [
    { status: 'IN_TRANSIT', label: 'Mark In Transit', variant: 'primary' },
    { status: 'CANCELLED', label: 'Cancel', variant: 'danger' },
  ],
  IN_TRANSIT: [
    { status: 'DELIVERED', label: 'Mark Delivered', variant: 'primary', requiresProof: true },
    { status: 'RETURNED', label: 'Mark Returned', variant: 'danger', isReturn: true },
  ],
};

export function formatStatus(status: DispatchStatus): string {
  return status.replaceAll('_', ' ');
}

// Matches PROMISED_DELIVERY_HOURS in src/logistics/dispatch/dispatch.rs — a
// dispatch is expected to reach DELIVERED within this many hours of being
// created. There's no per-order promised-date field yet, so this is one
// fleet-wide target; the backend's delay-alert scan (see
// docs/delay-alerts.md) uses the exact same threshold.
export const PROMISED_DELIVERY_HOURS = 72;

/** Still IN_TRANSIT, and past its promised delivery window. */
export function isRunningLate(order: Pick<DispatchOrder, 'status' | 'dispatched_at'>): boolean {
  if (order.status !== 'IN_TRANSIT') return false;
  const hoursSinceDispatch = (Date.now() / 1000 - order.dispatched_at) / 3600;
  return hoursSinceDispatch > PROMISED_DELIVERY_HOURS;
}

/** The happy path a dispatch moves along, in order (AWAITING_VEHICLE sits before it). */
export const LIFECYCLE_STEPS: DispatchStatus[] = ['PENDING', 'CONFIRMED', 'LOADED', 'IN_TRANSIT', 'DELIVERED'];

export interface LifecycleProgress {
  /** Index into LIFECYCLE_STEPS reached so far; -1 before PENDING. */
  step: number;
  /** Set when the dispatch left the happy path. */
  ended: 'returned' | 'cancelled' | null;
}

/**
 * Where a dispatch is on its lifecycle track. A return happens from
 * IN_TRANSIT, so it shows as reaching that step; a cancellation could come
 * from anywhere before delivery, so the last step its history reached is used.
 */
export function lifecycleProgress(order: Pick<DispatchOrder, 'status' | 'status_history'>): LifecycleProgress {
  const index = (s: DispatchStatus) => LIFECYCLE_STEPS.indexOf(s);
  if (order.status === 'RETURNED') return { step: index('IN_TRANSIT'), ended: 'returned' };
  if (order.status === 'CANCELLED') {
    const reached = Math.max(-1, ...(order.status_history ?? []).map(e => index(e.status)));
    return { step: reached, ended: 'cancelled' };
  }
  return { step: index(order.status), ended: null };
}
