jest.mock('@react-native-async-storage/async-storage', () =>
  require('@react-native-async-storage/async-storage/jest/async-storage-mock'),
);

import AsyncStorage from '@react-native-async-storage/async-storage';
import { readStatus, writeStatus } from '../statusStorage';

beforeEach(async () => {
  await AsyncStorage.clear();
});

describe('status storage', () => {
  it('returns the default status when nothing is stored', async () => {
    const status = await readStatus();
    expect(status).toMatchObject({ tracking: false, queueLength: 0, lastFlushOutcome: null });
  });

  it('merges a patch onto the existing status rather than replacing it', async () => {
    await writeStatus({ queueLength: 3 });
    const after = await writeStatus({ lastFlushOutcome: 'ok' });

    expect(after).toMatchObject({ queueLength: 3, lastFlushOutcome: 'ok' });
    await expect(readStatus()).resolves.toMatchObject({ queueLength: 3, lastFlushOutcome: 'ok' });
  });
});
