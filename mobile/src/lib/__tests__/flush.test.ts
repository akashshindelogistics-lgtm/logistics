import { DriverLocationApiError } from '../api';
import { flushQueue } from '../flush';
import type { QueuedFix } from '../types';

// Keep the real `DriverLocationApiError` class (its `.status` matters to
// `classify()` in flush.ts) and only replace `reportLocation` — a plain
// `jest.mock('../api')` automock would also replace the class with a mock
// constructor that never assigns `.status`.
jest.mock('../api', () => ({
  ...jest.requireActual('../api'),
  reportLocation: jest.fn(),
}));
// Plain `jest.mock(path)` automocking still requires (and evaluates) the
// real module to learn its shape, which would hit AsyncStorage's "native
// module is null" guard under Jest. An explicit factory avoids that.
jest.mock('../queueStorage', () => ({ loadQueue: jest.fn(), saveQueue: jest.fn() }));
jest.mock('../storage', () => ({ loadPairing: jest.fn() }));
jest.mock('../statusStorage', () => ({ readStatus: jest.fn(), writeStatus: jest.fn() }));

import { reportLocation } from '../api';
import { loadQueue, saveQueue } from '../queueStorage';
import { loadPairing } from '../storage';
import { writeStatus } from '../statusStorage';

const mockReportLocation = reportLocation as jest.MockedFunction<typeof reportLocation>;
const mockLoadQueue = loadQueue as jest.MockedFunction<typeof loadQueue>;
const mockSaveQueue = saveQueue as jest.MockedFunction<typeof saveQueue>;
const mockLoadPairing = loadPairing as jest.MockedFunction<typeof loadPairing>;
const mockWriteStatus = writeStatus as jest.MockedFunction<typeof writeStatus>;

const PAIRING = { deviceToken: 'token-1', apiBaseUrl: 'https://api.example.com' };
const FIX: QueuedFix = { id: 'a', latitude: 1, longitude: 1, recordedAt: 1_700_000_000 };

beforeEach(() => {
  jest.resetAllMocks();
  mockWriteStatus.mockResolvedValue({} as never);
});

describe('flushQueue', () => {
  it('does nothing when the phone is not paired', async () => {
    mockLoadPairing.mockResolvedValue(null);
    await flushQueue();
    expect(mockReportLocation).not.toHaveBeenCalled();
  });

  it('does nothing when the queue is empty, but records the length', async () => {
    mockLoadPairing.mockResolvedValue(PAIRING);
    mockLoadQueue.mockResolvedValue([]);
    await flushQueue();
    expect(mockReportLocation).not.toHaveBeenCalled();
    expect(mockWriteStatus).toHaveBeenCalledWith({ queueLength: 0 });
  });

  it('uploads the batch and removes exactly the sent fixes on success', async () => {
    mockLoadPairing.mockResolvedValue(PAIRING);
    mockLoadQueue.mockResolvedValue([FIX, { ...FIX, id: 'b' }]);
    mockReportLocation.mockResolvedValue({
      accepted: 2,
      location_updated: true,
      vehicle_registration_number: 'MH14 GP 0001',
      location: null,
    });

    await flushQueue();

    expect(mockReportLocation).toHaveBeenCalledWith(PAIRING.apiBaseUrl, PAIRING.deviceToken, [
      FIX,
      { ...FIX, id: 'b' },
    ]);
    expect(mockSaveQueue).toHaveBeenCalledWith([]);
    expect(mockWriteStatus).toHaveBeenCalledWith(
      expect.objectContaining({ queueLength: 0, lastFlushOutcome: 'ok' }),
    );
  });

  it('records "stale" without dropping fixes when the server had a newer position', async () => {
    mockLoadPairing.mockResolvedValue(PAIRING);
    mockLoadQueue.mockResolvedValue([FIX]);
    mockReportLocation.mockResolvedValue({
      accepted: 1,
      location_updated: false,
      vehicle_registration_number: 'MH14 GP 0001',
      location: null,
    });

    await flushQueue();

    // The (already-stale) fix is still removed — it was successfully
    // accepted by the server, just not applied.
    expect(mockSaveQueue).toHaveBeenCalledWith([]);
    expect(mockWriteStatus).toHaveBeenCalledWith(
      expect.objectContaining({ lastFlushOutcome: 'stale' }),
    );
  });

  it('leaves the queue untouched and classifies a 401 as unauthorized', async () => {
    mockLoadPairing.mockResolvedValue(PAIRING);
    mockLoadQueue.mockResolvedValue([FIX]);
    mockReportLocation.mockRejectedValue(new DriverLocationApiError(401, 'bad token'));

    await flushQueue();

    expect(mockSaveQueue).not.toHaveBeenCalled();
    expect(mockWriteStatus).toHaveBeenCalledWith(
      expect.objectContaining({ queueLength: 1, lastFlushOutcome: 'unauthorized' }),
    );
  });

  it.each([
    [403, 'inactive'],
    [409, 'no_vehicle'],
    [400, 'invalid_fix'],
    [0, 'network_error'],
    [500, 'server_error'],
  ])('classifies status %d as %s and keeps the queue', async (status, outcome) => {
    mockLoadPairing.mockResolvedValue(PAIRING);
    mockLoadQueue.mockResolvedValue([FIX]);
    mockReportLocation.mockRejectedValue(new DriverLocationApiError(status, 'x'));

    await flushQueue();

    expect(mockSaveQueue).not.toHaveBeenCalled();
    expect(mockWriteStatus).toHaveBeenCalledWith(expect.objectContaining({ lastFlushOutcome: outcome }));
  });
});
