let counter = 0;

/** A locally-unique id for a queued fix, good enough to remove exactly the
 * fixes an upload covered (see `queue.ts#removeByIds`). Not a UUID and not
 * sent to the server — it never leaves the device. */
export function generateLocalId(): string {
  counter = (counter + 1) % Number.MAX_SAFE_INTEGER;
  return `${Date.now()}-${counter}-${Math.random().toString(36).slice(2, 8)}`;
}
