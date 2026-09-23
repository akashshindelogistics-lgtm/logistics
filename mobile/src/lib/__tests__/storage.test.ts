jest.mock('expo-secure-store', () => ({
  setItemAsync: jest.fn(),
  getItemAsync: jest.fn(),
  deleteItemAsync: jest.fn(),
}));
// storage.ts also imports AsyncStorage (used on web only — see the comment
// there), so it needs a factory mock too, for the same reason every other
// AsyncStorage-adjacent test file needs one: automocking would otherwise
// evaluate the real module and hit its "native module is null" guard.
jest.mock('@react-native-async-storage/async-storage', () => ({
  setItem: jest.fn(),
  getItem: jest.fn(),
  removeItem: jest.fn(),
}));

import * as SecureStore from 'expo-secure-store';
import { clearPairing, loadPairing, savePairing } from '../storage';

const mockGet = SecureStore.getItemAsync as jest.MockedFunction<typeof SecureStore.getItemAsync>;
const mockSet = SecureStore.setItemAsync as jest.MockedFunction<typeof SecureStore.setItemAsync>;
const mockDelete = SecureStore.deleteItemAsync as jest.MockedFunction<typeof SecureStore.deleteItemAsync>;

beforeEach(() => {
  jest.resetAllMocks();
});

describe('pairing storage', () => {
  it('round-trips through SecureStore as JSON', async () => {
    const pairing = { deviceToken: 't', apiBaseUrl: 'https://api.example.com' };
    await savePairing(pairing);
    expect(mockSet).toHaveBeenCalledWith(expect.any(String), JSON.stringify(pairing));

    mockGet.mockResolvedValue(JSON.stringify(pairing));
    await expect(loadPairing()).resolves.toEqual(pairing);
  });

  it('returns null when nothing is stored', async () => {
    mockGet.mockResolvedValue(null);
    await expect(loadPairing()).resolves.toBeNull();
  });

  it('returns null for corrupted or shape-mismatched storage instead of throwing', async () => {
    mockGet.mockResolvedValue('not json');
    await expect(loadPairing()).resolves.toBeNull();

    mockGet.mockResolvedValue(JSON.stringify({ deviceToken: 't' })); // missing apiBaseUrl
    await expect(loadPairing()).resolves.toBeNull();
  });

  it('clears the stored pairing', async () => {
    await clearPairing();
    expect(mockDelete).toHaveBeenCalledWith(expect.any(String));
  });
});
