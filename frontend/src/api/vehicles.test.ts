import { describe, it, expect, vi, beforeEach } from 'vitest';
import api from './client';
import {
  listVehicles,
  addVehicle,
  updateVehicle,
  deleteVehicle,
  listVehicleDocuments,
  listOrgVehicleDocuments,
  addVehicleDocument,
  updateVehicleDocument,
  deleteVehicleDocument,
} from './vehicles';

vi.mock('./client', () => ({
  default: { get: vi.fn(), post: vi.fn(), put: vi.fn(), delete: vi.fn() },
}));

const envelope = <T,>(data: T) => ({ data: { success: true, message: '', data } });

describe('vehicles api client', () => {
  beforeEach(() => vi.resetAllMocks());

  it('listVehicles GETs /vehicles and unwraps', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope([{ registration_number: 'MH01AB1234' }]));
    const res = await listVehicles();
    expect(api.get).toHaveBeenCalledWith('/vehicles');
    expect(res.data).toEqual([{ registration_number: 'MH01AB1234' }]);
  });

  it('addVehicle POSTs to /orgs/{id}/vehicles with a hardcoded MetricTon unit', async () => {
    vi.mocked(api.post).mockResolvedValue(envelope({}));
    await addVehicle('o1', 'MH01AB1234', 12);
    expect(api.post).toHaveBeenCalledWith('/orgs/o1/vehicles', {
      registration_number: 'MH01AB1234',
      capacity: 12,
      unit: 'MetricTon',
    });
  });

  it('updateVehicle PUTs capacity/unit to /vehicles/{reg} (URL-encoded) and unwraps', async () => {
    vi.mocked(api.put).mockResolvedValue(envelope({ registration_number: 'MH 01', capacity: 30, unit: 'Box' }));
    const res = await updateVehicle('MH 01', 30, 'Box');
    expect(api.put).toHaveBeenCalledWith('/vehicles/MH%2001', { capacity: 30, unit: 'Box' });
    expect(res.data).toEqual({ registration_number: 'MH 01', capacity: 30, unit: 'Box' });
  });

  it('deleteVehicle URL-encodes the registration number', async () => {
    vi.mocked(api.delete).mockResolvedValue(envelope(null));
    await deleteVehicle('MH 01/AB 1234');
    expect(api.delete).toHaveBeenCalledWith('/vehicles/MH%2001%2FAB%201234');
  });

  it('deleteVehicle leaves a plain registration number untouched', async () => {
    vi.mocked(api.delete).mockResolvedValue(envelope(null));
    await deleteVehicle('MH01AB1234');
    expect(api.delete).toHaveBeenCalledWith('/vehicles/MH01AB1234');
  });

  it('listVehicleDocuments GETs the URL-encoded per-vehicle route', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope([]));
    await listVehicleDocuments('MH 01 AB 1234');
    expect(api.get).toHaveBeenCalledWith('/vehicles/MH%2001%20AB%201234/documents');
  });

  it('listOrgVehicleDocuments GETs the org-wide compliance route', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope([]));
    await listOrgVehicleDocuments('o1');
    expect(api.get).toHaveBeenCalledWith('/orgs/o1/vehicle-documents');
  });

  it('addVehicleDocument POSTs the payload to the per-vehicle route', async () => {
    vi.mocked(api.post).mockResolvedValue(envelope({}));
    await addVehicleDocument('MH01AB1234', {
      doc_type: 'Insurance',
      document_number: 'POL-1',
      expires_on: '2027-01-01',
    });
    expect(api.post).toHaveBeenCalledWith('/vehicles/MH01AB1234/documents', {
      doc_type: 'Insurance',
      document_number: 'POL-1',
      expires_on: '2027-01-01',
    });
  });

  it('updateVehicleDocument PUTs to /vehicle-documents/{id}', async () => {
    vi.mocked(api.put).mockResolvedValue(envelope({}));
    await updateVehicleDocument('d1', {
      doc_type: 'Permit',
      document_number: 'PMT-9',
      expires_on: '2028-06-30',
    });
    expect(api.put).toHaveBeenCalledWith('/vehicle-documents/d1', {
      doc_type: 'Permit',
      document_number: 'PMT-9',
      expires_on: '2028-06-30',
    });
  });

  it('deleteVehicleDocument DELETEs /vehicle-documents/{id}', async () => {
    vi.mocked(api.delete).mockResolvedValue(envelope(null));
    await deleteVehicleDocument('d1');
    expect(api.delete).toHaveBeenCalledWith('/vehicle-documents/d1');
  });
});
