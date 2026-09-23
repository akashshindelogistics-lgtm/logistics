import { DriverLocationApiError, reportLocation } from './api';
import { nextBatch, removeByIds } from './queue';
import { loadQueue, saveQueue } from './queueStorage';
import { loadPairing } from './storage';
import { writeStatus, type FlushOutcome } from './statusStorage';

function classify(err: unknown): { outcome: FlushOutcome; message: string } {
  if (err instanceof DriverLocationApiError) {
    switch (err.status) {
      case 0:
        return { outcome: 'network_error', message: 'No network connection — will retry.' };
      case 401:
        return { outcome: 'unauthorized', message: 'Device token was rejected — unpair and re-pair this phone.' };
      case 403:
        return { outcome: 'inactive', message: 'This driver is marked inactive — ask your dispatcher.' };
      case 409:
        return { outcome: 'no_vehicle', message: 'No vehicle is assigned to this driver yet.' };
      case 400:
        return { outcome: 'invalid_fix', message: err.message };
      default:
        return { outcome: 'server_error', message: err.message };
    }
  }
  return { outcome: 'server_error', message: err instanceof Error ? err.message : 'Unknown error' };
}

/**
 * Upload the oldest queued batch, if there is one. Safe to call often and
 * from anywhere — the background task calls it after every location update,
 * and the status screen's "Sync now" button calls it directly — since an
 * empty queue or a missing pairing is just a no-op.
 *
 * A batch the server rejects outright (401/403/409/400) is left in the
 * queue rather than discarded: 401/403/409 describe a standing problem
 * (bad token, inactive driver, unassigned vehicle) that resolves itself
 * once fixed on the dashboard, and the same fixes can be retried then. A
 * 400 should not occur in practice, since fixes are validated before being
 * queued (`validation.ts`) — leaving it queued surfaces the problem on the
 * status screen instead of silently losing the data.
 */
export async function flushQueue(): Promise<void> {
  const pairing = await loadPairing();
  if (!pairing) {
    return;
  }

  const queue = await loadQueue();
  if (queue.length === 0) {
    await writeStatus({ queueLength: 0 });
    return;
  }

  const batch = nextBatch(queue);
  try {
    const result = await reportLocation(pairing.apiBaseUrl, pairing.deviceToken, batch);
    const remaining = removeByIds(queue, batch.map((fix) => fix.id));
    await saveQueue(remaining);
    await writeStatus({
      queueLength: remaining.length,
      lastFlushAt: Date.now(),
      lastFlushOutcome: result.location_updated ? 'ok' : 'stale',
      lastFlushMessage: result.location_updated
        ? `Reported to ${result.vehicle_registration_number}`
        : 'The server already had a newer position for this batch',
      vehicleRegistrationNumber: result.vehicle_registration_number,
    });
  } catch (err) {
    const { outcome, message } = classify(err);
    await writeStatus({
      queueLength: queue.length,
      lastFlushAt: Date.now(),
      lastFlushOutcome: outcome,
      lastFlushMessage: message,
    });
  }
}
