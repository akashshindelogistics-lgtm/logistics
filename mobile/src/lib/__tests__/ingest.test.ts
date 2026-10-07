import { ingestFix } from '../ingest';
import type { DriverLocationFix } from '../types';

// Plain `jest.mock(path)` automocking still requires (and evaluates) the
// real module to learn its shape, which would hit AsyncStorage's "native
// module is null" guard under Jest. An explicit factory avoids that.
jest.mock('../queueStorage', () => ({ loadQueue: jest.fn(), saveQueue: jest.fn() }));
jest.mock('../statusStorage', () => ({ readStatus: jest.fn(), writeStatus: jest.fn() }));

import { loadQueue, saveQueue } from '../queueStorage';
import { writeStatus } from '../statusStorage';

const mockLoadQueue = loadQueue as jest.MockedFunction<typeof loadQueue>;
const mockSaveQueue = saveQueue as jest.MockedFunction<typeof saveQueue>;
const mockWriteStatus = writeStatus as jest.MockedFunction<typeof writeStatus>;

beforeEach(() => {
  jest.resetAllMocks();
  mockLoadQueue.mockResolvedValue([]);
  mockWriteStatus.mockResolvedValue({} as never);
});

const VALID_FIX: DriverLocationFix = {
  latitude: 18.5,
  longitude: 73.8,
  recordedAt: Math.floor(Date.now() / 1000) - 5,
};

describe('ingestFix', () => {
  it('queues a valid fix with a generated id', async () => {
    await ingestFix(VALID_FIX);

    expect(mockSaveQueue).toHaveBeenCalledTimes(1);
    const saved = mockSaveQueue.mock.calls[0][0];
    expect(saved).toHaveLength(1);
    expect(saved[0]).toMatchObject(VALID_FIX);
    expect(typeof saved[0].id).toBe('string');
    expect(saved[0].id.length).toBeGreaterThan(0);
    expect(mockWriteStatus).toHaveBeenCalledWith(
      expect.objectContaining({ queueLength: 1, lastFixAt: VALID_FIX.recordedAt }),
    );
  });

  it('drops an invalid fix instead of queueing it', async () => {
    await ingestFix({ ...VALID_FIX, latitude: 999 });

    expect(mockSaveQueue).not.toHaveBeenCalled();
    expect(mockWriteStatus).toHaveBeenCalledWith(
      expect.objectContaining({ lastFlushOutcome: 'invalid_fix' }),
    );
  });

  it('appends to whatever is already queued', async () => {
    mockLoadQueue.mockResolvedValue([
      { id: 'existing', latitude: 1, longitude: 1, recordedAt: 1 },
    ]);

    await ingestFix(VALID_FIX);

    const saved = mockSaveQueue.mock.calls[0][0];
    expect(saved).toHaveLength(2);
    expect(saved[0].id).toBe('existing');
  });
});
