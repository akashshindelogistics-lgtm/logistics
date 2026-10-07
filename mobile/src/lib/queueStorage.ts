import AsyncStorage from '@react-native-async-storage/async-storage';
import { STORAGE_KEYS } from './config';
import type { QueuedFix } from './types';

/** Persists the offline queue across app restarts and background-task runs.
 * AsyncStorage (not SecureStore) is fine here — fixes are not secret, and
 * the queue can be larger than SecureStore's per-item size limit. */

export async function loadQueue(): Promise<QueuedFix[]> {
  const raw = await AsyncStorage.getItem(STORAGE_KEYS.queue);
  if (!raw) {
    return [];
  }
  try {
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as QueuedFix[]) : [];
  } catch {
    return [];
  }
}

export async function saveQueue(queue: QueuedFix[]): Promise<void> {
  await AsyncStorage.setItem(STORAGE_KEYS.queue, JSON.stringify(queue));
}
