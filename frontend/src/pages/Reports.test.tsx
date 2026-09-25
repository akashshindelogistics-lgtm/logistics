import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
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
      { godown_id: 'g1', godown_name: 'North Godown', units_on_hand: 500, distinct_items: 3, capacity_used_percent: 42.5, category_breakdown: [{ category: 'General', units: 500 }] },
      { godown_id: 'g2', godown_name: 'South Godown', units_on_hand: 80, distinct_items: 1, capacity_used_percent: null, category_breakdown: [{ category: 'General', units: 80 }] },
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

  it('generates and displays an AI narration of the report on demand', async () => {
    vi.mocked(reportsApi.getOpsReport).mockResolvedValue(ok(report()));
    vi.mocked(reportsApi.getOpsReportSummary).mockResolvedValue(
      ok('Utilization is unusually low this week - only 1 of 4 vehicles are active.'),
    );

    const user = userEvent.setup();
    render(<Reports />);

    const button = await screen.findByRole('button', { name: /explain this report/i });
    await user.click(button);

    expect(reportsApi.getOpsReportSummary).toHaveBeenCalledWith('org1');
    expect(await screen.findByText(/utilization is unusually low this week/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /regenerate/i })).toBeInTheDocument();
  });

  it('shows a fallback message when the report summary call fails', async () => {
    vi.mocked(reportsApi.getOpsReport).mockResolvedValue(ok(report()));
    vi.mocked(reportsApi.getOpsReportSummary).mockRejectedValue(new Error('network error'));

    const user = userEvent.setup();
    render(<Reports />);

    await user.click(await screen.findByRole('button', { name: /explain this report/i }));

    expect(await screen.findByText(/ensure anthropic_api_key is set on the server/i)).toBeInTheDocument();
  });

  it('says so when nothing has gone out on hired trucks', async () => {
    vi.mocked(reportsApi.getOpsReport).mockResolvedValue(ok(report({
      hired_transport: {
        own_dispatches: 3, hired_dispatches: 0, hired_share_percent: 0, hire_cost_total: 0,
        paid_to_vendors: 0, outstanding_to_vendors: 0, awaiting_truck: 0, vendors: [], hire_margins: [],
      },
    })));
    render(<Reports />);
    expect(await screen.findByText(/no dispatches have gone out on hired trucks/i)).toBeInTheDocument();
  });

  it('shows hired share, vendor spend and margin per hire', async () => {
    vi.mocked(reportsApi.getOpsReport).mockResolvedValue(ok(report({
      hired_transport: {
        own_dispatches: 1, hired_dispatches: 2, hired_share_percent: 66.7, hire_cost_total: 9000,
        paid_to_vendors: 6000, outstanding_to_vendors: 3000, awaiting_truck: 1,
        vendors: [{ vendor_id: 'v1', vendor_name: 'Sharma Roadlines', hires: 1, hire_cost: 9000, paid: 6000, outstanding: 3000 }],
        hire_margins: [
          { hire_id: 'h1', vendor_name: 'Sharma Roadlines', registration_number: 'MH12 HR 1', trip_id: null,
            dispatches: 1, invoiced_dispatches: 1, invoiced: 12500, hire_cost: 9000, margin: 3500 },
          { hire_id: 'h2', vendor_name: 'Sharma Roadlines', registration_number: 'MH12 HR 2', trip_id: 't1',
            dispatches: 2, invoiced_dispatches: 1, invoiced: 4000, hire_cost: 7000, margin: -3000 },
        ],
      },
    })));
    render(<Reports />);

    const section = await screen.findByTestId('hired-transport');
    expect(section).toHaveTextContent('2 of 3 dispatches on hired trucks (66.7%)');
    expect(section).toHaveTextContent('1 awaiting a truck');
    expect(within(section).getByTestId('owed-to-vendors')).toHaveTextContent('3,000');
    const rows = within(section).getAllByTestId('hire-margin-row');
    expect(rows[0]).toHaveTextContent('3,500');
    expect(rows[1]).toHaveTextContent('Trip');
    expect(rows[1]).toHaveTextContent('(1/2 invoiced)');
    expect(rows[1]).toHaveTextContent('-3,000');
  });
});
