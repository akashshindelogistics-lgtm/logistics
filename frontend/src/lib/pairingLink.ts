/**
 * The link a driver's phone pairs from, shown as a QR code on the driver's
 * page. The Android app reads it from its own scanner or when the phone opens
 * the link, fills in the pairing form, and still waits for the driver to tap
 * Pair. Parsed by `parsePairingLink` in android/.../domain/PairingLink.kt.
 */
export const PAIRING_LINK_PREFIX = 'logisticsdriver://pair';

export const pairingLink = (server: string, token: string) =>
  `${PAIRING_LINK_PREFIX}?server=${encodeURIComponent(server)}&token=${encodeURIComponent(token)}`;

/** "just now", "5 min ago", "3 h ago", "2 days ago". */
export function timeAgo(unixSeconds: number, nowMs: number = Date.now()): string {
  const secs = Math.max(0, Math.round(nowMs / 1000 - unixSeconds));
  if (secs < 60) return 'just now';
  if (secs < 3600) return `${Math.floor(secs / 60)} min ago`;
  if (secs < 86_400) return `${Math.floor(secs / 3600)} h ago`;
  const days = Math.floor(secs / 86_400);
  return `${days} day${days === 1 ? '' : 's'} ago`;
}
