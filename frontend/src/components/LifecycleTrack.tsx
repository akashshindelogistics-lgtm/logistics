import type { DispatchOrder } from '../types';
import { LIFECYCLE_STEPS, formatStatus, lifecycleProgress } from '../lib/dispatchLifecycle';
import './LifecycleTrack.css';

/**
 * A slim five-step track (pending → delivered) under a dispatch's status tag.
 * The fill grows with a CSS transition when the status advances, the current
 * step pulses while the truck is in transit, and a return or cancellation
 * turns the track red at the step where it stopped.
 */
export default function LifecycleTrack({ order }: { order: Pick<DispatchOrder, 'status' | 'status_history'> }) {
  const { step, ended } = lifecycleProgress(order);
  const last = LIFECYCLE_STEPS.length - 1;
  const fill = Math.max(0, step) / last;
  const label = ended
    ? `${formatStatus(order.status)} after ${step >= 0 ? formatStatus(LIFECYCLE_STEPS[step]) : 'creation'}`
    : step >= 0 ? `Step ${step + 1} of ${last + 1}: ${formatStatus(LIFECYCLE_STEPS[step])}` : 'Waiting for a vehicle';

  return (
    <div
      className={`lifecycle-track${ended ? ` is-${ended}` : ''}${order.status === 'IN_TRANSIT' ? ' is-moving' : ''}`}
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={last + 1}
      aria-valuenow={step + 1}
      data-testid="lifecycle-track"
    >
      <span className="lifecycle-rail">
        <span className="lifecycle-fill" style={{ transform: `scaleX(${fill})` }} />
      </span>
      {LIFECYCLE_STEPS.map((s, i) => (
        <span
          key={s}
          className={`lifecycle-dot${i <= step ? ' is-done' : ''}${i === step ? ' is-current' : ''}`}
          style={{ left: `${(i / last) * 100}%` }}
          title={formatStatus(s)}
        />
      ))}
    </div>
  );
}
