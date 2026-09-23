import AsyncStorage from '@react-native-async-storage/async-storage';
import * as SecureStore from 'expo-secure-store';
import { Platform } from 'react-native';
import { STORAGE_KEYS } from './config';
import type { Pairing } from './types';

/**
 * The device token is a bearer credential (see `docs/driver-phone-tracking.md`)
 * — it belongs in the OS keychain/keystore via `expo-secure-store`, not in
 * AsyncStorage, which is unencrypted.
 *
 * `expo-secure-store` has no web implementation (its `ExpoSecureStore.web.js`
 * is an empty stub, so every method throws "is not a function") — found
 * while screenshotting this app's `expo start --web` preview. On web there
 * is no OS keychain to defer to anyway, so this falls back to AsyncStorage
 * there and to real SecureStore on iOS/Android, rather than crashing the one
 * platform this app doesn't ship a driver's phone on. `SecureStore` and
 * `AsyncStorage` don't share method names (`setItemAsync` vs. `setItem`), so
 * each is a tiny wrapper rather than one object swapped wholesale.
 */
const isWeb = Platform.OS === 'web';

function setItem(key: string, value: string): Promise<void> {
  return isWeb ? AsyncStorage.setItem(key, value) : SecureStore.setItemAsync(key, value);
}

function getItem(key: string): Promise<string | null> {
  return isWeb ? AsyncStorage.getItem(key) : SecureStore.getItemAsync(key);
}

function removeItem(key: string): Promise<void> {
  return isWeb ? AsyncStorage.removeItem(key) : SecureStore.deleteItemAsync(key);
}

export async function savePairing(pairing: Pairing): Promise<void> {
  await setItem(STORAGE_KEYS.pairing, JSON.stringify(pairing));
}

export async function loadPairing(): Promise<Pairing | null> {
  const raw = await getItem(STORAGE_KEYS.pairing);
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
  await removeItem(STORAGE_KEYS.pairing);
}
