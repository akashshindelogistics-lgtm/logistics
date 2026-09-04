import type { DispatchStatus } from '../types';

export const STATUS_TAG_CLASS: Record<DispatchStatus, string> = {
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
export const NEXT_ACTIONS: Partial<Record<DispatchStatus, NextAction[]>> = {
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
  return status.replace('_', ' ');
}
