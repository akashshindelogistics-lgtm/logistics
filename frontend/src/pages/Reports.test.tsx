import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import Reports from './Reports';
import * as reportsApi from '../api/reports';
import type { OpsReport } from '../types';

vi.mock('../api/reports');
vi.mock('../api/auth', () => ({ getOrgId: () => 'org1' }));

const ok = <T,>(data: T) => ({ success: true, message: '', data });

function report(overrides: Partial<OpsReport> = {}): OpsReport {
  return {
    vehicle_utilization: { total_vehicles: 4, vehicles_on_active_trip: 1, utilization_percent: 25 },
    delivery_performance: {
      delivered_count: 6, returned_count: 1, avg_hours_to_deliver: 30, on_time_rate_percent: 66.7,
    },
    units_dispatched_recently: 120,
    godown_inventory: [
      { godown_id: 'g1', godown_name: 'North Godown', units_on_hand: 500, distinct_items: 3, capacity_used_percent: 42.5 },
      { godown_id: 'g2', godown_name: 'South Godown', units_on_hand: 80, distinct_items: 1, capacity_used_percent: null },
    ],
    dispatch_volume: Array.from({ length: 14 }, (_, i) => ({
      date: `2026-09-${String(i + 1).padStart(2, '0')}`,
      count: i === 13 ? 5 : i % 3,
    })),
    ...overrides,
  };
}

describe('Reports page', () => {
  beforeEach(() => vi.resetAllMocks());

  it('renders the four headline tiles from the report', async () => {
    vi.mocked(reportsApi.getOpsReport).mockResolvedValue(ok(report()));
    render(<Reports />);

    const util = (await screen.findByText('Fleet utilization')).closest('.stat-card') as HTMLElement;
    expect(within(util).getByText('25.0%')).toBeInTheDocument();
    expect(within(util).getByText('1 of 4 on a trip')).toBeInTheDocument();

    const delivered = screen.getByText('Delivered').closest('.stat-card') as HTMLElement;
    expect(within(delivered).getByText('6')).toBeInTheDocument();
    expect(within(delivered).getByText('1 returned')).toBeInTheDocument();

    expect(screen.getByText('On-time rate').closest('.stat-card') as HTMLElement).toHaveTextContent('66.7%');
  });

  it('lists each godown with its on-hand units and capacity use', async () => {
    vi.mocked(reportsApi.getOpsReport).mockResolvedValue(ok(report()));
    render(<Reports />);

    const north = (await screen.findByText('North Godown')).closest('tr')!;
    expect(within(north).getByText('500')).toBeInTheDocument();
    expect(within(north).getByText('42.5%')).toBeInTheDocument();

    const south = screen.getByText('South Godown').closest('tr')!;
    expect(within(south).getByText('no cap')).toBeInTheDocument();
  });

  it('renders a bar per day of the dispatch-volume window', async () => {
    vi.mocked(reportsApi.getOpsReport).mockResolvedValue(ok(report()));
    render(<Reports />);

    expect(await screen.findByText(/dispatch volume/i)).toBeInTheDocument();
    // 14 day cells, each titled "YYYY-MM-DD: count".
    const bars = screen.getAllByTitle(/^\d{4}-\d{2}-\d{2}: \d+$/);
    expect(bars).toHaveLength(14);
    expect(screen.getByText('120 units dispatched in 30 days')).toBeInTheDocument();
  });

  it('shows an empty state when the report cannot be loaded', async () => {
    vi.mocked(reportsApi.getOpsReport).mockRejectedValue(new Error('boom'));
    render(<Reports />);
    expect(await screen.findByText(/no report available/i)).toBeInTheDocument();
  });
});
