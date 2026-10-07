import AsyncStorage from '@react-native-async-storage/async-storage';
import { STORAGE_KEYS } from './config';

/** Why the last upload attempt ended the way it did — shown on the status
 * screen so a driver (or whoever supports them) can tell "offline, will
 * retry" apart from "this needs a dispatcher to fix". */
export type FlushOutcome =
  | 'ok'
  | 'stale'
  | 'network_error'
  | 'unauthorized'
  | 'inactive'
  | 'no_vehicle'
  | 'invalid_fix'
  | 'server_error';

export interface TrackingStatus {
  tracking: boolean;
  queueLength: number;
  /** Unix seconds (the phone's clock), from the newest fix captured so far. */
  lastFixAt: number | null;
  /** Milliseconds since epoch (`Date.now()`), when the last upload attempt
   * finished — used only for display, so it doesn't need to match the
   * server's or a fix's clock. */
  lastFlushAt: number | null;
  lastFlushOutcome: FlushOutcome | null;
  lastFlushMessage: string | null;
  vehicleRegistrationNumber: string | null;
}

const DEFAULT_STATUS: TrackingStatus = {
  tracking: false,
  queueLength: 0,
  lastFixAt: null,
  lastFlushAt: null,
  lastFlushOutcome: null,
  lastFlushMessage: null,
  vehicleRegistrationNumber: null,
};

export async function readStatus(): Promise<TrackingStatus> {
  const raw = await AsyncStorage.getItem(STORAGE_KEYS.status);
  if (!raw) {
    return DEFAULT_STATUS;
  }
  try {
    return { ...DEFAULT_STATUS, ...JSON.parse(raw) };
  } catch {
    return DEFAULT_STATUS;
  }
}

/** Merge `patch` into the persisted status (read-modify-write — there is no
 * concurrent writer other than this module, since the background task and
 * the UI both go through these same functions rather than holding state of
 * their own). Returns the merged status. */
export async function writeStatus(patch: Partial<TrackingStatus>): Promise<TrackingStatus> {
  const current = await readStatus();
  const next = { ...current, ...patch };
  await AsyncStorage.setItem(STORAGE_KEYS.status, JSON.stringify(next));
  return next;
}
