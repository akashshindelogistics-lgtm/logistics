import { MAX_FIXES_PER_REQUEST, MAX_QUEUED_FIXES } from './config';
import type { QueuedFix } from './types';

/**
 * Pure operations on the offline queue. Kept free of AsyncStorage / Expo
 * imports so they can be unit-tested as plain data transforms; the storage
 * wrapper in `queueStorage.ts` is the only place that persists the result.
 */

/** Append a fix, oldest-first. If the queue is already at
 * `MAX_QUEUED_FIXES`, the oldest fix is dropped to make room — a phone that
 * has been offline for a very long time loses its earliest history rather
 * than growing the queue without bound. */
export function enqueue(queue: QueuedFix[], fix: QueuedFix): QueuedFix[] {
  const next = [...queue, fix];
  if (next.length <= MAX_QUEUED_FIXES) {
    return next;
  }
  return next.slice(next.length - MAX_QUEUED_FIXES);
}

/** The oldest `MAX_FIXES_PER_REQUEST` fixes, to send as one upload. Does not
 * remove them from the queue — call `removeByIds` once the upload succeeds. */
export function nextBatch(queue: QueuedFix[]): QueuedFix[] {
  return queue.slice(0, MAX_FIXES_PER_REQUEST);
}

/** Drop exactly the fixes whose `id` is in `ids` (an uploaded batch),
 * leaving anything queued afterwards — including fixes queued by the
 * background task while the upload was in flight — untouched. */
export function removeByIds(queue: QueuedFix[], ids: readonly string[]): QueuedFix[] {
  if (ids.length === 0) {
    return queue;
  }
  const toRemove = new Set(ids);
  return queue.filter((fix) => !toRemove.has(fix.id));
}
