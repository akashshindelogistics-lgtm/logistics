import * as SecureStore from 'expo-secure-store';
import { STORAGE_KEYS } from './config';
import type { Pairing } from './types';

/**
 * The device token is a bearer credential (see `docs/driver-phone-tracking.md`)
 * — it belongs in the OS keychain/keystore via `expo-secure-store`, not in
 * AsyncStorage, which is unencrypted.
 */

export async function savePairing(pairing: Pairing): Promise<void> {
  await SecureStore.setItemAsync(STORAGE_KEYS.pairing, JSON.stringify(pairing));
}

export async function loadPairing(): Promise<Pairing | null> {
  const raw = await SecureStore.getItemAsync(STORAGE_KEYS.pairing);
  if (!raw) {
    return null;
  }
  try {
    const parsed = JSON.parse(raw);
    if (typeof parsed?.deviceToken === 'string' && typeof parsed?.apiBaseUrl === 'string') {
      return parsed as Pairing;
    }
  } catch {
    // Corrupted storage is treated the same as "never paired".
  }
  return null;
}

export async function clearPairing(): Promise<void> {
  await SecureStore.deleteItemAsync(STORAGE_KEYS.pairing);
}
