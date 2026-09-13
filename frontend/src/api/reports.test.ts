import { describe, it, expect, vi, beforeEach } from 'vitest';
import api from './client';
import { getOpsReport, getOpsReportSummary } from './reports';

vi.mock('./client', () => ({ default: { get: vi.fn() } }));

const envelope = <T,>(data: T) => ({ data: { success: true, message: '', data } });

describe('reports api client', () => {
  beforeEach(() => vi.resetAllMocks());

  it('getOpsReport GETs /orgs/{id}/reports and unwraps the envelope', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope({ units_dispatched_recently: 7 }));
    const res = await getOpsReport('org1');
    expect(api.get).toHaveBeenCalledWith('/orgs/org1/reports');
    expect(res.data).toEqual({ units_dispatched_recently: 7 });
  });

  it('getOpsReportSummary GETs /orgs/{id}/reports/summary and unwraps the envelope', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope('Utilization is unusually low this week.'));
    const res = await getOpsReportSummary('org1');
    expect(api.get).toHaveBeenCalledWith('/orgs/org1/reports/summary');
    expect(res.data).toBe('Utilization is unusually low this week.');
  });
});
