import { describe, it, expect, vi, beforeEach } from 'vitest';
import api from './client';
import { listDispatches, getDispatchSummary, updateDispatchStatus } from './dispatches';

vi.mock('./client', () => ({
  default: {
    get: vi.fn(),
    put: vi.fn(),
    post: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('dispatches api client', () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it('listDispatches GETs /dispatches', async () => {
    vi.mocked(api.get).mockResolvedValue({ data: { success: true, message: '', data: [] } });
    await listDispatches();
    expect(api.get).toHaveBeenCalledWith('/dispatches');
  });

  it('getDispatchSummary GETs /dispatches/{id}/summary', async () => {
    vi.mocked(api.get).mockResolvedValue({ data: { success: true, message: '', data: 'a summary' } });
    await getDispatchSummary('abc-123');
    expect(api.get).toHaveBeenCalledWith('/dispatches/abc-123/summary');
  });

  it('updateDispatchStatus PUTs the status with no proof for a non-delivery move', async () => {
    vi.mocked(api.put).mockResolvedValue({ data: { success: true, message: '', data: {} } });
    await updateDispatchStatus('abc-123', 'CONFIRMED');
    expect(api.put).toHaveBeenCalledWith('/dispatches/abc-123/status', {
      status: 'CONFIRMED',
      proof_of_delivery: undefined,
    });
  });

  it('updateDispatchStatus PUTs proof_of_delivery when marking DELIVERED', async () => {
    vi.mocked(api.put).mockResolvedValue({ data: { success: true, message: '', data: {} } });
    await updateDispatchStatus('abc-123', 'DELIVERED', {
      receiver_name: 'Priya Sharma',
      signature_or_photo_url: 'https://example.com/sig.png',
    });
    expect(api.put).toHaveBeenCalledWith('/dispatches/abc-123/status', {
      status: 'DELIVERED',
      proof_of_delivery: {
        receiver_name: 'Priya Sharma',
        signature_or_photo_url: 'https://example.com/sig.png',
      },
    });
  });

  it('updateDispatchStatus PUTs return_to_godown_id when marking RETURNED', async () => {
    vi.mocked(api.put).mockResolvedValue({ data: { success: true, message: '', data: {} } });
    await updateDispatchStatus('abc-123', 'RETURNED', undefined, 'godown-9');
    expect(api.put).toHaveBeenCalledWith('/dispatches/abc-123/status', {
      status: 'RETURNED',
      proof_of_delivery: undefined,
      return_to_godown_id: 'godown-9',
    });
  });
});
