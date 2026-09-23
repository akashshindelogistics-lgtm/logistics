jest.mock('@react-native-async-storage/async-storage', () =>
  require('@react-native-async-storage/async-storage/jest/async-storage-mock'),
);

import AsyncStorage from '@react-native-async-storage/async-storage';
import { STORAGE_KEYS } from '../config';
import { loadQueue, saveQueue } from '../queueStorage';
import type { QueuedFix } from '../types';

beforeEach(async () => {
  await AsyncStorage.clear();
});

describe('queue storage', () => {
  it('returns an empty array when nothing is stored', async () => {
    await expect(loadQueue()).resolves.toEqual([]);
  });

  it('round-trips a queue through AsyncStorage', async () => {
    const queue: QueuedFix[] = [{ id: 'a', latitude: 1, longitude: 1, recordedAt: 1 }];
    await saveQueue(queue);
    await expect(loadQueue()).resolves.toEqual(queue);
  });

  it('treats corrupted or non-array storage as an empty queue', async () => {
    await AsyncStorage.setItem(STORAGE_KEYS.queue, 'not json');
    await expect(loadQueue()).resolves.toEqual([]);

    await AsyncStorage.setItem(STORAGE_KEYS.queue, JSON.stringify({ not: 'an array' }));
    await expect(loadQueue()).resolves.toEqual([]);
  });
});
