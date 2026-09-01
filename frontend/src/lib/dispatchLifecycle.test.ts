import { describe, it, expect } from 'vitest';
import { NEXT_ACTIONS, STATUS_TAG_CLASS, formatStatus } from './dispatchLifecycle';
import type { DispatchStatus } from '../types';

const ALL_STATUSES: DispatchStatus[] = [
  'PENDING',
  'CONFIRMED',
  'LOADED',
  'IN_TRANSIT',
  'DELIVERED',
  'RETURNED',
  'CANCELLED',
];

describe('NEXT_ACTIONS', () => {
  // This mapping is a UI convenience only (which buttons to show) — the
  // backend's DispatchStatus::can_transition_to is what actually enforces
  // the state machine. Kept here so a drift between the two shows up as a
  // failing test instead of a silently wrong/missing button.
  it('mirrors DispatchStatus::can_transition_to', () => {
    expect(NEXT_ACTIONS.PENDING?.map(a => a.status)).toEqual(['CONFIRMED', 'CANCELLED']);
    expect(NEXT_ACTIONS.CONFIRMED?.map(a => a.status)).toEqual(['LOADED', 'CANCELLED']);
    expect(NEXT_ACTIONS.LOADED?.map(a => a.status)).toEqual(['IN_TRANSIT', 'CANCELLED']);
    expect(NEXT_ACTIONS.IN_TRANSIT?.map(a => a.status)).toEqual(['DELIVERED', 'RETURNED']);
  });

  it('requires proof of delivery only for the move to DELIVERED', () => {
    expect(NEXT_ACTIONS.IN_TRANSIT?.find(a => a.status === 'DELIVERED')?.requiresProof).toBe(true);
    expect(NEXT_ACTIONS.IN_TRANSIT?.find(a => a.status === 'RETURNED')?.requiresProof).toBeFalsy();
    for (const status of ALL_STATUSES) {
      for (const action of NEXT_ACTIONS[status] ?? []) {
        if (action.status !== 'DELIVERED') {
          expect(action.requiresProof).toBeFalsy();
        }
      }
    }
  });

  it('offers no action for a terminal status', () => {
    expect(NEXT_ACTIONS.DELIVERED).toBeUndefined();
    expect(NEXT_ACTIONS.RETURNED).toBeUndefined();
    expect(NEXT_ACTIONS.CANCELLED).toBeUndefined();
  });
});

describe('STATUS_TAG_CLASS', () => {
  it('has a tag class for every status', () => {
    for (const status of ALL_STATUSES) {
      expect(STATUS_TAG_CLASS[status]).toMatch(/^tag-/);
    }
  });
});

describe('formatStatus', () => {
  it('replaces the underscore with a space', () => {
    expect(formatStatus('IN_TRANSIT')).toBe('IN TRANSIT');
  });

  it('leaves single-word statuses unchanged', () => {
    expect(formatStatus('PENDING')).toBe('PENDING');
    expect(formatStatus('DELIVERED')).toBe('DELIVERED');
  });
});
