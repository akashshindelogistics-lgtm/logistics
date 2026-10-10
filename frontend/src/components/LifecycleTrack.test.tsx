import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import LifecycleTrack from './LifecycleTrack';
import { lifecycleProgress } from '../lib/dispatchLifecycle';
import type { DispatchStatus } from '../types';

const order = (status: DispatchStatus, history: DispatchStatus[] = []) => ({
  status,
  status_history: history.map((s, i) => ({ status: s, changed_at: i })),
});

describe('lifecycleProgress', () => {
  it('maps each happy-path status to its step', () => {
    expect(lifecycleProgress(order('AWAITING_VEHICLE'))).toEqual({ step: -1, ended: null });
    expect(lifecycleProgress(order('PENDING'))).toEqual({ step: 0, ended: null });
    expect(lifecycleProgress(order('IN_TRANSIT'))).toEqual({ step: 3, ended: null });
    expect(lifecycleProgress(order('DELIVERED'))).toEqual({ step: 4, ended: null });
  });

  it('places a return at in-transit and a cancellation at the last step it reached', () => {
    expect(lifecycleProgress(order('RETURNED'))).toEqual({ step: 3, ended: 'returned' });
    expect(lifecycleProgress(order('CANCELLED', ['PENDING', 'CONFIRMED', 'CANCELLED']))).toEqual({ step: 1, ended: 'cancelled' });
    expect(lifecycleProgress(order('CANCELLED', ['AWAITING_VEHICLE', 'CANCELLED']))).toEqual({ step: -1, ended: 'cancelled' });
  });
});

describe('LifecycleTrack', () => {
  it('fills to the current step and describes it for screen readers', () => {
    render(<LifecycleTrack order={order('LOADED')} />);
    const track = screen.getByRole('progressbar');
    expect(track).toHaveAttribute('aria-valuenow', '3');
    expect(track).toHaveAttribute('aria-valuemax', '5');
    expect(track).toHaveAccessibleName('Step 3 of 5: LOADED');
    expect(track.querySelectorAll('.lifecycle-dot.is-done')).toHaveLength(3);
    expect(track.querySelector<HTMLElement>('.lifecycle-fill')!.style.transform).toBe('scaleX(0.5)');
  });

  it('pulses while in transit and turns red when returned', () => {
    const { rerender } = render(<LifecycleTrack order={order('IN_TRANSIT')} />);
    expect(screen.getByRole('progressbar')).toHaveClass('is-moving');
    rerender(<LifecycleTrack order={order('RETURNED')} />);
    expect(screen.getByRole('progressbar')).toHaveClass('is-returned');
    expect(screen.getByRole('progressbar')).not.toHaveClass('is-moving');
    expect(screen.getByRole('progressbar')).toHaveAccessibleName('RETURNED after IN TRANSIT');
  });
});
