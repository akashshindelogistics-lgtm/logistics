import { generateLocalId } from './id';
import { enqueue } from './queue';
import { loadQueue, saveQueue } from './queueStorage';
import { writeStatus } from './statusStorage';
import type { DriverLocationFix } from './types';
import { validateFix } from './validation';

/**
 * Validate and queue one fix captured by the OS location callback. Called
 * from the background task in `locationTask.ts` for every update the OS
 * delivers.
 *
 * An invalid fix (bad clock, coordinates off Earth) is dropped rather than
 * queued — see `validation.ts` for why queuing it would just delay the same
 * rejection to upload time, where it would also block every *valid* fix
 * queued alongside it (the server rejects a batch atomically).
 */
export async function ingestFix(fix: DriverLocationFix): Promise<void> {
  const nowSeconds = Math.floor(Date.now() / 1000);
  const reason = validateFix(fix, nowSeconds);
  if (reason) {
    await writeStatus({
      lastFlushOutcome: 'invalid_fix',
      lastFlushMessage: `Dropped a fix from the device: ${reason}`,
    });
    return;
  }

  const queue = await loadQueue();
  const next = enqueue(queue, { ...fix, id: generateLocalId() });
  await saveQueue(next);
  await writeStatus({ queueLength: next.length, lastFixAt: fix.recordedAt });
}
